use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::{Arc, LazyLock, OnceLock},
    time::Duration,
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{any, get},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use db::models::mcp_gateway::McpGatewayConnection;
use deployment::Deployment;
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use services::services::mcp_gateway_secrets::McpGatewaySecretStore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use ts_rs::TS;
use utils::{assets::mcp_gateway_key_path, response::ApiResponse};
use uuid::Uuid;

use crate::DeploymentImpl;

mod proxy;

static SECRETS: OnceLock<Result<McpGatewaySecretStore, String>> = OnceLock::new();
static REFRESH_LOCKS: LazyLock<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(|| tokio::sync::Mutex::new(HashMap::new()));
static ALLOWED_WORKER_PEERS: LazyLock<HashSet<IpAddr>> = LazyLock::new(|| {
    services::services::cluster::config::parse_worker_source_addresses(
        &std::env::var(services::services::cluster::config::WORKER_SOURCE_ADDRESSES_ENV)
            .unwrap_or_default(),
    )
    .unwrap_or_default()
    .into_iter()
    .collect()
});

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_endpoint: Option<String>,
    pub revocation_endpoint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub resource: String,
    pub cf_access_client_id: Option<String>,
    pub cf_access_client_secret: Option<String>,
}

fn secret_store() -> Result<&'static McpGatewaySecretStore, StatusCode> {
    SECRETS
        .get_or_init(|| {
            McpGatewaySecretStore::load_or_generate(&mcp_gateway_key_path())
                .map_err(|_| "MCP gateway secret storage is unavailable".to_string())
        })
        .as_ref()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

pub fn gateway_router() -> Router<DeploymentImpl> {
    Router::new().route("/mcp-gateway/{id}", any(proxy_request))
}

pub fn management_router() -> Router<DeploymentImpl> {
    Router::new().route(
        "/mcp-gateway/connections/{id}",
        get(status).delete(disconnect),
    )
}

async fn proxy_request(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    request: Request,
) -> Response {
    // The capability is defense-in-depth for process isolation. Cluster workers
    // are admitted only when deployment names their exact source address; the
    // public listener and arbitrary LAN peers retain the fail-closed 404.
    if !gateway_peer_allowed(peer.ip(), &ALLOWED_WORKER_PEERS) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(token) = bearer(request.headers()) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let Some(connection) = McpGatewayConnection::find_bound(
        &deployment.db().pool,
        &id,
        deployment.user_id(),
        deployment.user_id(),
    )
    .await
    .ok()
    .flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let digest = Sha256::digest(token.as_bytes());
    if digest
        .as_slice()
        .ct_eq(&connection.gateway_token_hash)
        .unwrap_u8()
        != 1
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(envelope) = connection.encrypted_credentials.as_deref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Shared MCP server is disconnected",
        )
            .into_response();
    };
    let binding = binding(&connection);
    let credentials = secret_store()
        .and_then(|store| {
            store
                .decrypt(envelope, binding.as_bytes())
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        })
        .and_then(|bytes| {
            serde_json::from_slice(&bytes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        });
    let mut credentials: StoredCredentials = match credentials {
        Ok(credentials) => credentials,
        Err(status) => return status.into_response(),
    };
    let prepared = match proxy::prepare(request).await {
        Ok(request) => request,
        Err(response) => return response,
    };
    let first = proxy::forward(&prepared, &connection, &credentials).await;
    if !matches!(
        first.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return first;
    }
    match refresh_credentials(&deployment, &connection, &credentials).await {
        Ok(refreshed) => {
            credentials = refreshed;
            proxy::forward(&prepared, &connection, &credentials).await
        }
        Err(_) => first,
    }
}

fn gateway_peer_allowed(peer: IpAddr, allowed_workers: &HashSet<IpAddr>) -> bool {
    peer.is_loopback() || allowed_workers.contains(&peer)
}

async fn refresh_credentials(
    deployment: &DeploymentImpl,
    original: &McpGatewayConnection,
    stale: &StoredCredentials,
) -> Result<StoredCredentials, String> {
    let lock = {
        let mut locks = REFRESH_LOCKS.lock().await;
        locks
            .entry(original.id.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;
    let current = McpGatewayConnection::find_bound(
        &deployment.db().pool,
        &original.id,
        deployment.user_id(),
        deployment.user_id(),
    )
    .await
    .map_err(|_| "refresh state unavailable".to_string())?
    .ok_or_else(|| "shared connection no longer exists".to_string())?;
    let envelope = current
        .encrypted_credentials
        .as_deref()
        .ok_or_else(|| "shared connection is disconnected".to_string())?;
    let raw = secret_store()
        .map_err(|_| "secret storage unavailable".to_string())?
        .decrypt(envelope, binding(&current).as_bytes())
        .map_err(|_| "stored credential unavailable".to_string())?;
    let mut credentials: StoredCredentials =
        serde_json::from_slice(&raw).map_err(|_| "stored credential unavailable".to_string())?;
    if credentials.access_token != stale.access_token {
        return Ok(credentials);
    }
    let refresh_token = credentials
        .refresh_token
        .as_deref()
        .ok_or_else(|| "no refresh token is available".to_string())?;
    let token_endpoint = credentials
        .token_endpoint
        .as_deref()
        .ok_or_else(|| "no token endpoint is available".to_string())?;
    let client_id = credentials
        .client_id
        .as_deref()
        .ok_or_else(|| "no OAuth client id is available".to_string())?;
    let client =
        executors::mcp_oauth::OAuthHttpClient::new(Duration::from_secs(15), &current.upstream_url)?;
    let client = match (
        credentials.cf_access_client_id.clone(),
        credentials.cf_access_client_secret.clone(),
    ) {
        (Some(id), Some(secret)) => client.with_cloudflare_access(id, secret),
        _ => client,
    };
    let tokens = executors::mcp_oauth::refresh_access_token(
        &client,
        token_endpoint,
        client_id,
        credentials.client_secret.as_deref(),
        refresh_token,
        &credentials.resource,
    )
    .await?;
    credentials.access_token = tokens.access_token;
    credentials.refresh_token = tokens.refresh_token;
    let raw =
        serde_json::to_vec(&credentials).map_err(|_| "credential update failed".to_string())?;
    let encrypted = secret_store()
        .map_err(|_| "secret storage unavailable".to_string())?
        .encrypt(&raw, binding(&current).as_bytes())
        .map_err(|_| "credential update failed".to_string())?;
    let updated = sqlx::query(
        r#"UPDATE mcp_gateway_connections
           SET encrypted_credentials = ?, credential_version = credential_version + 1,
               status = 'connected', last_error_code = NULL,
               updated_at = datetime('now', 'subsec')
           WHERE id = ? AND credential_version = ? AND status != 'disconnected'"#,
    )
    .bind(encrypted)
    .bind(&current.id)
    .bind(current.credential_version)
    .execute(&deployment.db().pool)
    .await
    .map_err(|_| "credential update failed".to_string())?;
    if updated.rows_affected() != 1 {
        return Err("credential changed concurrently".to_string());
    }
    Ok(credentials)
}

fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|v| !v.is_empty())
}

fn binding(connection: &McpGatewayConnection) -> String {
    format!(
        "{}|{}|{}|{}",
        connection.owner_user_id, connection.machine_id, connection.id, connection.upstream_url
    )
}

/// Persist a completed OAuth exchange and return the local capability only to
/// the caller that immediately writes agent-native gateway entries.
pub(crate) struct NewOAuthConnection<'a> {
    pub id: Uuid,
    pub server_name: &'a str,
    pub upstream_url: &'a str,
    pub transport: &'a str,
    pub tokens: executors::mcp_oauth::OAuthTokenSet,
    pub token_endpoint: String,
    pub revocation_endpoint: Option<String>,
    pub client_id: String,
    pub resource: String,
    pub cf_access_client_id: Option<String>,
    pub cf_access_client_secret: Option<String>,
    pub existing_gateway_token: Option<String>,
}

pub(crate) async fn store_oauth_connection(
    deployment: &DeploymentImpl,
    connection: NewOAuthConnection<'_>,
) -> Result<(String, String), String> {
    let NewOAuthConnection {
        id,
        server_name,
        upstream_url,
        transport,
        tokens,
        token_endpoint,
        revocation_endpoint,
        client_id,
        resource,
        cf_access_client_id,
        cf_access_client_secret,
        existing_gateway_token,
    } = connection;
    let parsed = reqwest::Url::parse(upstream_url)
        .ok()
        .filter(acceptable_upstream)
        .ok_or_else(|| "Upstream MCP URL must be HTTPS (or loopback HTTP)".to_string())?;
    let server_addr = deployment
        .client_info()
        .get_server_addr()
        .ok_or_else(|| "Could not determine the local MCP gateway port".to_string())?;
    let port = server_addr.port();
    let gateway_host = if server_addr.is_ipv6() {
        "[::1]"
    } else {
        "127.0.0.1"
    };
    let gateway_token = existing_gateway_token.unwrap_or_else(|| {
        let mut random = [0_u8; 32];
        OsRng.fill_bytes(&mut random);
        URL_SAFE_NO_PAD.encode(random)
    });
    let token_hash = Sha256::digest(gateway_token.as_bytes());
    let credentials = StoredCredentials {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_endpoint: Some(token_endpoint),
        revocation_endpoint,
        client_id: Some(client_id),
        client_secret: None,
        resource,
        cf_access_client_id,
        cf_access_client_secret,
    };
    let upstream_url = parsed.to_string();
    let binding = format!(
        "{}|{}|{}|{}",
        deployment.user_id(),
        deployment.user_id(),
        id,
        upstream_url
    );
    let raw = serde_json::to_vec(&credentials)
        .map_err(|_| "Failed to encode shared MCP credentials".to_string())?;
    let encrypted = secret_store()
        .map_err(|_| "MCP gateway secret storage is unavailable".to_string())?
        .encrypt(&raw, binding.as_bytes())
        .map_err(|_| "Failed to encrypt shared MCP credentials".to_string())?;
    sqlx::query(
        r#"INSERT INTO mcp_gateway_connections
           (id, owner_user_id, machine_id, server_name, upstream_url, transport,
            auth_kind, gateway_token_hash, encrypted_credentials, status, connected_at)
           VALUES (?, ?, ?, ?, ?, ?, 'oauth', ?, ?, 'connected', datetime('now', 'subsec'))
           ON CONFLICT(id) DO UPDATE SET
             server_name=excluded.server_name, upstream_url=excluded.upstream_url,
             transport=excluded.transport, gateway_token_hash=excluded.gateway_token_hash,
             encrypted_credentials=excluded.encrypted_credentials,
             credential_version=mcp_gateway_connections.credential_version+1,
             status='connected', last_error_code=NULL,
             connected_at=datetime('now', 'subsec'), updated_at=datetime('now', 'subsec')
           WHERE owner_user_id=excluded.owner_user_id AND machine_id=excluded.machine_id"#,
    )
    .bind(id.to_string())
    .bind(deployment.user_id())
    .bind(deployment.user_id())
    .bind(server_name)
    .bind(&upstream_url)
    .bind(transport)
    .bind(token_hash.as_slice())
    .bind(encrypted)
    .execute(&deployment.db().pool)
    .await
    .map_err(|_| "Failed to store shared MCP connection".to_string())?;
    Ok((
        format!("http://{gateway_host}:{port}/mcp-gateway/{id}"),
        gateway_token,
    ))
}

#[derive(Debug, Serialize, TS)]
pub struct GatewayConnectionResponse {
    pub id: Uuid,
    pub endpoint: String,
    pub status: String,
    pub has_refresh_token: bool,
}

async fn status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Json<ApiResponse<GatewayConnectionResponse>> {
    match McpGatewayConnection::find_bound(
        &deployment.db().pool,
        &id,
        deployment.user_id(),
        deployment.user_id(),
    )
    .await
    {
        Ok(Some(row)) => {
            let has_refresh_token = row
                .encrypted_credentials
                .as_deref()
                .and_then(|envelope| {
                    secret_store()
                        .ok()?
                        .decrypt(envelope, binding(&row).as_bytes())
                        .ok()
                })
                .and_then(|raw| serde_json::from_slice::<StoredCredentials>(&raw).ok())
                .and_then(|credentials| credentials.refresh_token)
                .is_some();
            Json(ApiResponse::success(GatewayConnectionResponse {
                id: Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::nil()),
                endpoint: deployment
                    .client_info()
                    .get_server_addr()
                    .map(|addr| {
                        let host = if addr.is_ipv6() { "[::1]" } else { "127.0.0.1" };
                        format!("http://{host}:{}/mcp-gateway/{}", addr.port(), row.id)
                    })
                    .unwrap_or_default(),
                status: row.status,
                has_refresh_token,
            }))
        }
        _ => Json(ApiResponse::error("Shared MCP connection not found")),
    }
}

async fn disconnect(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Json<ApiResponse<bool>> {
    if let Ok(Some(row)) = McpGatewayConnection::find_bound(
        &deployment.db().pool,
        &id,
        deployment.user_id(),
        deployment.user_id(),
    )
    .await
        && let Some(envelope) = row.encrypted_credentials.as_deref()
        && let Ok(raw) = secret_store().and_then(|store| {
            store
                .decrypt(envelope, binding(&row).as_bytes())
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        })
        && let Ok(credentials) = serde_json::from_slice::<StoredCredentials>(&raw)
        && let Some(endpoint) = credentials.revocation_endpoint.as_deref()
        && let Ok(client) =
            executors::mcp_oauth::OAuthHttpClient::new(Duration::from_secs(15), &row.upstream_url)
    {
        let client = match (
            credentials.cf_access_client_id.clone(),
            credentials.cf_access_client_secret.clone(),
        ) {
            (Some(id), Some(secret)) => client.with_cloudflare_access(id, secret),
            _ => client,
        };
        // Best effort by design: local deletion below always wins. Never log
        // provider bodies or token values when remote revocation fails.
        if let Some(refresh_token) = credentials.refresh_token.as_deref() {
            let _ = executors::mcp_oauth::revoke_token(
                &client,
                endpoint,
                credentials.client_id.as_deref(),
                credentials.client_secret.as_deref(),
                refresh_token,
                "refresh_token",
            )
            .await;
        }
        let _ = executors::mcp_oauth::revoke_token(
            &client,
            endpoint,
            credentials.client_id.as_deref(),
            credentials.client_secret.as_deref(),
            &credentials.access_token,
            "access_token",
        )
        .await;
    }
    match McpGatewayConnection::disconnect(
        &deployment.db().pool,
        &id,
        deployment.user_id(),
        deployment.user_id(),
    )
    .await
    {
        Ok(value) => Json(ApiResponse::success(value)),
        Err(_) => Json(ApiResponse::error(
            "Failed to disconnect shared MCP connection",
        )),
    }
}

fn acceptable_upstream(url: &reqwest::Url) -> bool {
    if url.scheme() == "https" {
        return url.username().is_empty() && url.password().is_none();
    }
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_peers_are_limited_to_loopback_and_exact_configured_workers() {
        let allowed = HashSet::from(["172.16.100.103".parse().unwrap()]);

        assert!(gateway_peer_allowed("127.0.0.1".parse().unwrap(), &allowed));
        assert!(gateway_peer_allowed("::1".parse().unwrap(), &allowed));
        assert!(gateway_peer_allowed(
            "172.16.100.103".parse().unwrap(),
            &allowed
        ));
        assert!(!gateway_peer_allowed(
            "172.16.100.104".parse().unwrap(),
            &allowed
        ));
        assert!(!gateway_peer_allowed(
            "198.51.100.10".parse().unwrap(),
            &allowed
        ));
    }

    #[test]
    fn worker_peer_allowlist_uses_explicit_source_addresses() {
        assert_eq!(
            services::services::cluster::config::parse_worker_source_addresses(
                "172.16.100.103,172.16.100.104"
            )
            .unwrap()
            .into_iter()
            .collect::<HashSet<_>>(),
            HashSet::from([
                "172.16.100.103".parse().unwrap(),
                "172.16.100.104".parse().unwrap()
            ])
        );
    }
}
