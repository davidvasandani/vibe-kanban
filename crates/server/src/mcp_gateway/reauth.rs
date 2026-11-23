//! Auto-auth for the gateway's upstream grant. Agent capabilities never rotate.
use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    Json,
    body::to_bytes,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use db::models::mcp_gateway::McpGatewayConnection;
use deployment::Deployment;
use executors::mcp_oauth;
use futures_util::{StreamExt, stream};
use serde_json::{Value, json};
use services::services::{
    entra_mint::Progress,
    reauth::{
        ReauthAuthState, ReauthError,
        gateway::{self, GatewayReauthProvider, GatewayTarget},
        sweep,
    },
};

use super::{
    REFRESH_LOCKS, StoredCredentials, binding, persist_credentials, proxy, refresh_credentials,
    secret_store,
};
use crate::DeploymentImpl;

pub(crate) fn register(deployment: DeploymentImpl) {
    gateway::register(Arc::new(Provider { deployment }));
}
struct Provider {
    deployment: DeploymentImpl,
}

fn failed(message: &str) -> ReauthError {
    ReauthError::Failed(message.into())
}

pub(super) fn token_expiry(expires_in: Option<u64>, token: &str) -> Option<String> {
    if let Some(seconds) = expires_in {
        return i64::try_from(seconds)
            .ok()
            .and_then(chrono::Duration::try_seconds)
            .and_then(|d| Utc::now().checked_add_signed(d))
            .map(|d| d.to_rfc3339());
    }
    // An unverified JWT claim is only a scheduling hint, never authentication.
    let payload = URL_SAFE_NO_PAD.decode(token.split('.').nth(1)?).ok()?;
    let value: Value = serde_json::from_slice(&payload).ok()?;
    DateTime::from_timestamp(value.get("exp")?.as_i64()?, 0).map(|d| d.to_rfc3339())
}
fn expiry(row: &McpGatewayConnection, credentials: &StoredCredentials) -> Option<DateTime<Utc>> {
    row.expires_at
        .clone()
        .or_else(|| token_expiry(None, &credentials.access_token))
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc))
}
pub(super) fn refresh_before_request(
    row: &McpGatewayConnection,
    credentials: &StoredCredentials,
) -> bool {
    credentials.refresh_token.is_some()
        && expiry(row, credentials)
            .is_some_and(|at| at <= Utc::now() + chrono::Duration::seconds(60))
}

fn due(at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    let horizon = sweep::sweep_interval_from_env()
        .unwrap_or(Duration::from_secs(1800))
        .as_secs()
        .saturating_add(300)
        .min(i64::MAX as u64) as i64;
    at.is_some_and(|at| at <= now + chrono::Duration::seconds(horizon))
}
fn credentials(row: &McpGatewayConnection) -> Result<StoredCredentials, ReauthError> {
    let raw = secret_store()
        .map_err(|_| failed("Gateway secret storage unavailable"))?
        .decrypt(
            row.encrypted_credentials
                .as_deref()
                .ok_or_else(|| failed("Gateway entry is disconnected"))?,
            binding(row).as_bytes(),
        )
        .map_err(|_| failed("Gateway credential could not be decrypted"))?;
    serde_json::from_slice(&raw).map_err(|_| failed("Gateway credential could not be decoded"))
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
fn target_name(row: &McpGatewayConnection, rows: &[McpGatewayConnection]) -> String {
    if safe_name(&row.server_name)
        && rows
            .iter()
            .filter(|r| r.server_name == row.server_name || r.id == row.server_name)
            .count()
            == 1
    {
        row.server_name.clone()
    } else {
        row.id.clone()
    }
}
async fn bound_rows(
    pool: &sqlx::SqlitePool,
    owner: &str,
    machine: &str,
) -> Result<Vec<McpGatewayConnection>, ReauthError> {
    sqlx::query_as::<_, McpGatewayConnection>("SELECT * FROM mcp_gateway_connections WHERE owner_user_id = ? AND machine_id = ? AND auth_kind IN ('oauth', 'cloudflare_service_token_oauth') AND status != 'disconnected' AND encrypted_credentials IS NOT NULL ORDER BY id")
            .bind(owner).bind(machine).fetch_all(pool).await.map_err(|_| failed("Gateway inventory unavailable"))
}

impl Provider {
    async fn rows(&self) -> Result<Vec<McpGatewayConnection>, ReauthError> {
        bound_rows(
            &self.deployment.db().pool,
            self.deployment.user_id(),
            self.deployment.user_id(),
        )
        .await
    }
    async fn resolve(&self, name: &str) -> Result<(McpGatewayConnection, String), ReauthError> {
        let rows = self.rows().await?;
        let matches: Vec<_> = rows
            .iter()
            .filter(|r| r.id == name || target_name(r, &rows) == name)
            .collect();
        if matches.len() != 1 {
            return Err(ReauthError::InvalidTarget(
                "Gateway entry is absent or ambiguous; use its UUID from list_reauth_targets"
                    .into(),
            ));
        }
        Ok((matches[0].clone(), target_name(matches[0], &rows)))
    }
}

#[derive(Debug, PartialEq)]
enum Probe {
    Authenticated,
    Rejected,
    Unknown,
}
fn initialized(value: &Value) -> bool {
    value.get("id") == Some(&json!(1))
        && value.get("error").is_none()
        && value
            .pointer("/result/protocolVersion")
            .and_then(Value::as_str)
            .is_some()
        && value
            .pointer("/result/serverInfo")
            .is_some_and(Value::is_object)
        && value
            .pointer("/result/capabilities")
            .is_some_and(Value::is_object)
}
fn initialized_body(bytes: &[u8]) -> bool {
    if serde_json::from_slice::<Value>(bytes).is_ok_and(|v| initialized(&v)) {
        return true;
    }
    String::from_utf8_lossy(bytes)
        .split('\n')
        .filter_map(|l| l.strip_prefix("data:"))
        .any(|s| serde_json::from_str::<Value>(s.trim()).is_ok_and(|v| initialized(&v)))
}
async fn probe(row: &McpGatewayConnection, credentials: &StoredCredentials) -> Probe {
    tokio::time::timeout(Duration::from_secs(12), async {
        let Ok(client) = proxy::validated_client(&row.upstream_url).await else { return Probe::Unknown; };
        let authorize = |request: reqwest::RequestBuilder| {
            let mut request = request.bearer_auth(&credentials.access_token);
            if let (Some(id), Some(secret)) = (&credentials.cf_access_client_id, &credentials.cf_access_client_secret) {
                request = request.header("CF-Access-Client-Id", id).header("CF-Access-Client-Secret", secret);
            }
            request
        };
        let response = authorize(client.post(&row.upstream_url))
            .header("accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"vk-auto-auth","version":"1"}}}))
            .send().await;
        let Ok(response) = response else { return Probe::Unknown; };
        if response.status() == reqwest::StatusCode::UNAUTHORIZED { return Probe::Rejected; }
        // A forbidden response may be a permission or edge-policy failure.
        let status = response.status();
        let session = response.headers().get("mcp-session-id").cloned();
        let mut chunks = response.bytes_stream();
        let mut bytes = Vec::new();
        let result = loop {
            match chunks.next().await {
                Some(Ok(chunk)) if bytes.len() + chunk.len() <= 65536 => bytes.extend_from_slice(&chunk),
                Some(_) => break Probe::Unknown,
                None => break if status == reqwest::StatusCode::FORBIDDEN && auth_kind(&bytes).is_some() { Probe::Rejected } else { Probe::Unknown },
            }
            if status.is_success() && initialized_body(&bytes) { break Probe::Authenticated; }
        };
        if let Some(session) = session {
            let _ = authorize(client.delete(&row.upstream_url)).header("mcp-session-id", session).timeout(Duration::from_secs(2)).send().await;
        }
        result
    }).await.unwrap_or(Probe::Unknown)
}
fn auth_kind(bytes: &[u8]) -> Option<&'static str> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    match value
        .get("kind")
        .or_else(|| value.get("error"))
        .and_then(Value::as_str)
    {
        Some("token_expired") => Some("token_expired"),
        Some("invalid_token") => Some("invalid_token"),
        _ => None,
    }
}

#[async_trait]
impl GatewayReauthProvider for Provider {
    async fn canonicalize(&self, name: &str) -> Result<String, ReauthError> {
        self.resolve(name).await.map(|(_, name)| name)
    }
    async fn targets(&self) -> Result<Vec<GatewayTarget>, ReauthError> {
        let rows = self.rows().await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let entries: Vec<_> = rows
            .iter()
            .map(|row| (row.clone(), target_name(row, &rows)))
            .collect();
        let targets = stream::iter(entries.into_iter().map(|(row, name)| async move {
            let (state, message, renewal_due) = match credentials(&row) {
                Err(_) => (ReauthAuthState::Unknown, Some("Gateway credential storage unavailable".into()), false),
                Ok(credentials) => {
                    let at = expiry(&row, &credentials);
                    // Probe even future expiry: a revoked token can still carry
                    // an unexpired timestamp, and an unsigned JWT is not proof.
                    match tokio::time::timeout_at(deadline, probe(&row, &credentials)).await.unwrap_or(Probe::Unknown) {
                        Probe::Authenticated => (ReauthAuthState::Authenticated, None, due(at, Utc::now())),
                        Probe::Rejected => (ReauthAuthState::Unauthenticated, Some("Upstream rejected the gateway credential".into()), true),
                        Probe::Unknown => (ReauthAuthState::Unknown, Some("Initialize could not verify authentication (transport, permissions, or protocol failure)".into()), false),
                    }
                }
            };
            GatewayTarget { name, label: format!("MCP gateway: {}", row.server_name), auth_state: state, auth_message: message, renewal_due }
        })).buffer_unordered(8).collect().await;
        Ok(targets)
    }
    async fn renew(&self, name: &str, progress: &Progress) -> Result<(), ReauthError> {
        let (row, _) = self.resolve(name).await?;
        let stale = credentials(&row)?;
        if stale.refresh_token.is_some() {
            progress.say("Refreshing the gateway's upstream OAuth grant");
            match refresh_credentials(&self.deployment.db().pool, &row, &stale).await {
                Ok(_) => return Ok(()),
                Err(error) if refresh_rejected(&error) => progress
                    .say("The refresh grant was rejected; trying the existing browser sign-in"),
                Err(_) => {
                    return Err(failed(
                        "Gateway refresh failed; retry after the token endpoint or credential storage recovers",
                    ));
                }
            }
        }
        // Share the proxy refresh lock, and re-read after acquiring it. Never
        // overwrite a manual reconnect or revive a disconnected connection.
        let lock = {
            let mut locks = REFRESH_LOCKS.lock().await;
            locks
                .entry(row.id.clone())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = lock.lock().await;
        let (current, _) = self.resolve(&row.id).await?;
        if current.credential_version != row.credential_version {
            return Ok(());
        }
        let mut stored = credentials(&current)?;
        let client = oauth_client(&current, &stored)
            .map_err(|_| failed("Gateway OAuth client configuration is invalid"))?;
        let metadata = mcp_oauth::discover(&client, &current.upstream_url, None)
            .await
            .map_err(|_| failed("Gateway OAuth discovery failed"))?;
        let client_id = stored.client_id.clone().ok_or_else(|| {
            ReauthError::NotConfigured(
                "Gateway has no stored OAuth client; reconnect in MCP settings".into(),
            )
        })?;
        let redirect = match &stored.redirect_uri {
            Some(uri) => uri.clone(),
            None => legacy_redirect(&self.deployment)?,
        };
        let pkce = mcp_oauth::Pkce::generate();
        let state = mcp_oauth::generate_state();
        let scopes: Vec<String> = stored
            .scope
            .as_deref()
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let url = mcp_oauth::build_authorize_url(
            &metadata.authorization_endpoint,
            &client_id,
            &redirect,
            &pkce.challenge,
            &state,
            &metadata.resource,
            &scopes,
        )
        .map_err(|_| failed("Gateway authorize URL is invalid"))?;
        let code = gateway::authorize_code(name, &url, &redirect, &state, progress).await?;
        let tokens = mcp_oauth::exchange_token_set(
            &client,
            &metadata.token_endpoint,
            &client_id,
            &code,
            &pkce.verifier,
            &redirect,
            &metadata.resource,
        )
        .await
        .map_err(|_| failed("Gateway authorization code exchange failed"))?;
        let expires_at = token_expiry(tokens.expires_in, &tokens.access_token);
        stored.scope = tokens.scope.or(stored.scope);
        stored.access_token = tokens.access_token;
        stored.refresh_token = tokens.refresh_token;
        stored.token_endpoint = Some(metadata.token_endpoint);
        stored.revocation_endpoint = metadata.revocation_endpoint;
        stored.resource = metadata.resource;
        stored.redirect_uri = Some(redirect);
        persist_credentials(&self.deployment.db().pool, &current, &stored, expires_at)
            .await
            .map_err(|_| failed("Gateway credential changed or could not be saved"))
    }
    async fn verify(&self, name: &str) -> Result<(), String> {
        let (row, _) = self.resolve(name).await.map_err(|e| e.to_string())?;
        let credentials = credentials(&row).map_err(|e| e.to_string())?;
        match probe(&row, &credentials).await {
            Probe::Authenticated => Ok(()),
            Probe::Rejected => Err("Gateway initialize still rejects the credential".into()),
            Probe::Unknown => Err(
                "Gateway initialize could not be verified; transport or protocol failure".into(),
            ),
        }
    }
}
fn oauth_client(
    row: &McpGatewayConnection,
    stored: &StoredCredentials,
) -> Result<mcp_oauth::OAuthHttpClient, String> {
    let client = mcp_oauth::OAuthHttpClient::new(Duration::from_secs(15), &row.upstream_url)?;
    Ok(
        match (&stored.cf_access_client_id, &stored.cf_access_client_secret) {
            (Some(id), Some(secret)) => client.with_cloudflare_access(id.clone(), secret.clone()),
            _ => client,
        },
    )
}
fn refresh_rejected(error: &str) -> bool {
    matches!(
        error,
        "token refresh rejected: invalid_grant"
            | "token refresh rejected: invalid_token"
            | "token refresh rejected: invalid_client"
            | "token refresh rejected: unauthorized_client"
    )
}
fn legacy_redirect(deployment: &DeploymentImpl) -> Result<String, ReauthError> {
    if let Ok(base) = std::env::var("MCP_OAUTH_PUBLIC_BASE_URL") {
        let mut url = reqwest::Url::parse(&base)
            .map_err(|_| failed("Invalid configured OAuth callback base"))?;
        if url.scheme() != "https" || url.host_str().is_none() {
            return Err(failed("Invalid configured OAuth callback base"));
        }
        url.set_path("/api/mcp-auth/callback");
        url.set_query(None);
        url.set_fragment(None);
        return Ok(url.to_string());
    }
    let addr = deployment
        .client_info()
        .get_server_addr()
        .ok_or_else(|| failed("Gateway callback port unavailable"))?;
    Ok(format!(
        "http://localhost:{}/api/mcp-auth/callback",
        addr.port()
    ))
}

pub(super) async fn annotate_auth_error(
    deployment: &DeploymentImpl,
    row: &McpGatewayConnection,
    response: Response,
) -> Response {
    if !matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return response;
    }
    let provider = Provider {
        deployment: deployment.clone(),
    };
    let target = provider
        .resolve(&row.id)
        .await
        .map(|(_, name)| name)
        .unwrap_or_else(|_| row.id.clone());
    auth_error_response(response, &target).await
}

async fn auth_error_response(response: Response, target: &str) -> Response {
    let (mut parts, body) = response.into_parts();
    let bytes = match tokio::time::timeout(Duration::from_secs(5), to_bytes(body, 65536)).await {
        Ok(Ok(bytes)) => bytes,
        _ => {
            return (
                StatusCode::BAD_GATEWAY,
                "Upstream authentication response could not be read",
            )
                .into_response();
        }
    };
    let kind = auth_kind(&bytes);
    if kind.is_none() && parts.status != StatusCode::UNAUTHORIZED {
        return Response::from_parts(parts, bytes.into());
    }
    let Some(mut value) = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(Value::is_object)
    else {
        return Response::from_parts(parts, bytes.into());
    };
    value["reauth_target"] = json!(format!("mcp-gateway:{target}"));
    if let Some(kind) = kind {
        value["kind"] = json!(kind);
    }
    parts.headers.remove("content-length");
    parts
        .headers
        .insert("content-type", "application/json".parse().unwrap());
    let body = Json(value).into_response().into_body();
    Response::from_parts(parts, body)
}

#[cfg(test)]
#[path = "reauth/tests.rs"]
mod integration_tests;
