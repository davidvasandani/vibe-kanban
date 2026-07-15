use std::{net::IpAddr, sync::OnceLock};

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{any, get, post},
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

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct StoredCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_endpoint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
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
    Router::new()
        .route("/mcp-gateway/connections", post(upsert_connection))
        .route("/mcp-gateway/connections/{id}", get(status).delete(disconnect))
}

async fn proxy_request(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    request: Request,
) -> Response {
    // The capability is defense-in-depth for local process isolation; never
    // expose this route to a non-loopback peer even if the main UI listener is.
    if !request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|host| host.split(':').next())
        .is_some_and(|host| host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()))
    {
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
    .flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let digest = Sha256::digest(token.as_bytes());
    if digest.as_slice().ct_eq(&connection.gateway_token_hash).unwrap_u8() != 1 {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Some(envelope) = connection.encrypted_credentials.as_deref() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Shared MCP server is disconnected").into_response();
    };
    let binding = binding(&connection);
    let credentials = secret_store()
        .and_then(|store| store.decrypt(envelope, binding.as_bytes()).map_err(|_| StatusCode::SERVICE_UNAVAILABLE))
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|_| StatusCode::SERVICE_UNAVAILABLE));
    match credentials {
        Ok(credentials) => proxy::forward(request, &connection, credentials).await,
        Err(status) => status.into_response(),
    }
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

#[derive(Debug, Deserialize)]
pub struct UpsertConnectionRequest {
    pub id: Option<Uuid>,
    pub server_name: String,
    pub upstream_url: String,
    pub transport: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_endpoint: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub cf_access_client_id: Option<String>,
    pub cf_access_client_secret: Option<String>,
}

#[derive(Debug, Serialize, TS)]
pub struct GatewayConnectionResponse {
    pub id: Uuid,
    pub endpoint: String,
    pub gateway_token: Option<String>,
    pub status: String,
    pub has_refresh_token: bool,
}

async fn upsert_connection(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<UpsertConnectionRequest>,
) -> Json<ApiResponse<GatewayConnectionResponse>> {
    let parsed = match reqwest::Url::parse(&payload.upstream_url) {
        Ok(url) if acceptable_upstream(&url) => url,
        _ => return Json(ApiResponse::error("Upstream MCP URL must be HTTPS (or loopback HTTP)")),
    };
    if !matches!(payload.transport.as_str(), "http" | "sse") {
        return Json(ApiResponse::error("Gateway transport must be http or sse"));
    }
    let id = payload.id.unwrap_or_else(Uuid::new_v4);
    let mut gateway_token = [0_u8; 32];
    OsRng.fill_bytes(&mut gateway_token);
    let gateway_token = URL_SAFE_NO_PAD.encode(gateway_token);
    let token_hash = Sha256::digest(gateway_token.as_bytes());
    let auth_kind = if payload.cf_access_client_id.is_some() {
        "cloudflare_service_token_oauth"
    } else {
        "oauth"
    };
    let credentials = StoredCredentials {
        access_token: payload.access_token,
        refresh_token: payload.refresh_token,
        token_endpoint: payload.token_endpoint,
        client_id: payload.client_id,
        client_secret: payload.client_secret,
        cf_access_client_id: payload.cf_access_client_id,
        cf_access_client_secret: payload.cf_access_client_secret,
    };
    let upstream_url = parsed.to_string();
    let binding = format!("{}|{}|{}|{}", deployment.user_id(), deployment.user_id(), id, upstream_url);
    let encrypted = match secret_store().and_then(|store| {
        serde_json::to_vec(&credentials)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
            .and_then(|raw| store.encrypt(&raw, binding.as_bytes()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR))
    }) {
        Ok(value) => value,
        Err(_) => return Json(ApiResponse::error("MCP gateway secret storage is unavailable")),
    };
    let result = sqlx::query(
        r#"INSERT INTO mcp_gateway_connections
           (id, owner_user_id, machine_id, server_name, upstream_url, transport,
            auth_kind, gateway_token_hash, encrypted_credentials, status, connected_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'connected', datetime('now', 'subsec'))
           ON CONFLICT(id) DO UPDATE SET
             server_name=excluded.server_name, upstream_url=excluded.upstream_url,
             transport=excluded.transport, auth_kind=excluded.auth_kind,
             gateway_token_hash=excluded.gateway_token_hash,
             encrypted_credentials=excluded.encrypted_credentials,
             credential_version=mcp_gateway_connections.credential_version+1,
             status='connected', last_error_code=NULL,
             connected_at=datetime('now', 'subsec'), updated_at=datetime('now', 'subsec')
           WHERE owner_user_id=excluded.owner_user_id AND machine_id=excluded.machine_id"#,
    )
    .bind(id.to_string())
    .bind(deployment.user_id())
    .bind(deployment.user_id())
    .bind(&payload.server_name)
    .bind(&upstream_url)
    .bind(&payload.transport)
    .bind(auth_kind)
    .bind(token_hash.as_slice())
    .bind(encrypted)
    .execute(&deployment.db().pool)
    .await;
    if result.is_err() {
        return Json(ApiResponse::error("Failed to store shared MCP connection"));
    }
    Json(ApiResponse::success(GatewayConnectionResponse {
        id,
        endpoint: format!("http://127.0.0.1/mcp-gateway/{id}"),
        gateway_token: Some(gateway_token),
        status: "connected".to_string(),
        has_refresh_token: credentials.refresh_token.is_some(),
    }))
}

async fn status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Json<ApiResponse<GatewayConnectionResponse>> {
    match McpGatewayConnection::find_bound(&deployment.db().pool, &id, deployment.user_id(), deployment.user_id()).await {
        Ok(Some(row)) => Json(ApiResponse::success(GatewayConnectionResponse {
            id: Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::nil()),
            endpoint: format!("http://127.0.0.1/mcp-gateway/{}", row.id),
            gateway_token: None,
            status: row.status,
            has_refresh_token: false,
        })),
        _ => Json(ApiResponse::error("Shared MCP connection not found")),
    }
}

async fn disconnect(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Json<ApiResponse<bool>> {
    match McpGatewayConnection::disconnect(&deployment.db().pool, &id).await {
        Ok(value) => Json(ApiResponse::success(value)),
        Err(_) => Json(ApiResponse::error("Failed to disconnect shared MCP connection")),
    }
}

fn acceptable_upstream(url: &reqwest::Url) -> bool {
    if url.scheme() == "https" {
        return url.username().is_empty() && url.password().is_none();
    }
    url.scheme() == "http"
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().and_then(|h| h.parse::<IpAddr>().ok()).is_some_and(|ip| ip.is_loopback())
}
