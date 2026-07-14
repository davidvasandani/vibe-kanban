//! OAuth "Connect" flow for auth-required MCP servers.
//!
//! The MCP settings screen probes servers via `/mcp-config/test`; a 401/403
//! yields `auth_required`. These routes let the user fix that from the UI:
//! `start` discovers the server's authorization server (RFC 9728/8414),
//! registers a client (RFC 7591), and returns an authorization URL the
//! frontend opens in a popup; the browser redirect lands on `callback`, which
//! exchanges the code (PKCE) and writes the token into the agent's own config
//! file as an `Authorization` header — the same place a user would paste one
//! by hand — then the frontend polls `status` and re-tests the server.
//!
//! Pending flows are process-local, short-lived, in-memory state (see the
//! feature plan's R4): keyed by flow id, bound to an unguessable `state`
//! value, expired after [`FLOW_TTL`], and their exchange inputs are consumed
//! on first callback so a code/state pair cannot be redeemed twice. Access
//! tokens are never stored here, never logged, and never returned in JSON.

use std::{
    collections::HashMap,
    env,
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Json as ResponseJson, Response},
    routing::{get, post},
};
use deployment::Deployment;
use executors::{
    executors::{BaseCodingAgent, StandardCodingAgentExecutor},
    mcp_config::read_agent_config,
    mcp_oauth,
    profile::{ExecutorConfigs, ExecutorProfileId},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    routes::{
        config::{get_mcp_servers_from_config_path, update_mcp_servers_in_config},
        oauth::{close_window_response, simple_html_response},
    },
};

/// Abandoned flows are unusable after this long (pruned on access).
const FLOW_TTL: Duration = Duration::from_secs(600);
const MAX_FLOWS: usize = 256;

/// Per-request bound for every outbound OAuth call (discovery, DCR, code
/// exchange) so a stalled endpoint can't hang `start` or `callback` — the
/// same discipline as the probe's `MCP_TEST_TIMEOUT`.
const OAUTH_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// Enrich a dynamic-client-registration failure with actionable guidance.
///
/// Some authorization servers only accept redirect URIs belonging to a
/// hardcoded allowlist of known MCP clients (Claude, ChatGPT/Codex, Cursor) or
/// to `localhost`. A server-hosted Vibe Kanban reached through a public host
/// registers a non-loopback callback those servers reject — the raw error
/// ("redirect_uri must be a trusted … callback") is opaque, so spell out the
/// two ways forward.
fn connect_error(raw: String, redirect_uri: &str) -> String {
    let looks_like_redirect_rejection = {
        let lower = raw.to_ascii_lowercase();
        lower.contains("redirect_uri")
            || lower.contains("redirect uri")
            // Remote bodies are intentionally redacted. A public DCR HTTP 400
            // can no longer be classified from its description, so retain the
            // safe loopback/manual-token guidance for that common case.
            || (lower.contains("client registration failed") && lower.contains("http 400"))
    };
    let is_loopback = redirect_uri.contains("://localhost")
        || redirect_uri.contains("://127.0.0.1")
        || redirect_uri.contains("://[::1]");
    if looks_like_redirect_rejection && !is_loopback {
        format!(
            "{raw}\n\nThis authorization server only trusts callbacks from known \
             MCP clients or localhost, and Vibe Kanban registered \
             `{redirect_uri}`. Open Vibe Kanban on localhost (so the callback \
             is a loopback URL this server accepts) and click Connect there, or \
             obtain a token another way and paste it into this server's headers \
             (Authorization: Bearer …) via the edit dialog."
        )
    } else {
        raw
    }
}

#[derive(Debug, Deserialize)]
pub struct McpAuthQuery {
    executor: BaseCodingAgent,
}

#[derive(TS, Debug, Deserialize)]
pub struct McpAuthStartRequest {
    pub server_name: String,
    /// The `WWW-Authenticate` header a probe captured for this server, if
    /// any. Some servers only issue their challenge (and its
    /// `resource_metadata` pointer) on the JSON-RPC POST the probe makes —
    /// passing it here lets discovery start from it instead of hoping a
    /// plain GET re-elicits one.
    #[serde(default)]
    pub www_authenticate: Option<String>,
    /// Register a `http://localhost:<port>` callback instead of deriving it
    /// from the request host. Authorization servers with a strict allowlist
    /// (Claude/ChatGPT/Codex/Cursor/localhost) reject a public callback; a
    /// loopback one is accepted. When the browser can reach that loopback
    /// (same machine or an SSH port-forward) the callback completes
    /// automatically; otherwise the user pastes the full redirected URL back
    /// via `/mcp-auth/complete`.
    #[serde(default)]
    pub loopback: bool,
}

#[derive(TS, Debug, Serialize)]
pub struct McpAuthStartResponse {
    pub flow_id: Uuid,
    pub authorize_url: String,
    /// True when this flow used a loopback callback, so the frontend knows to
    /// offer manual callback-URL entry if the popup can't reach it.
    pub loopback: bool,
}

#[derive(TS, Debug, Deserialize)]
pub struct McpAuthCompleteRequest {
    pub flow_id: Uuid,
    /// Full redirect URL the browser landed on, including both `code` and
    /// `state` (`http://localhost:…/callback?code=…&state=…`).
    pub code: String,
}

#[derive(TS, Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpAuthFlowState {
    Pending,
    Completed,
    Failed,
}

#[derive(TS, Debug, Serialize)]
pub struct McpAuthStatusResponse {
    pub status: McpAuthFlowState,
    pub error: Option<String>,
}

/// What the callback needs to redeem the authorization code. Held separately
/// from the flow record and `take()`n under the write lock so a state value
/// can only be exchanged once.
#[derive(Debug)]
struct ExchangeInputs {
    oauth_client: mcp_oauth::OAuthHttpClient,
    pkce_verifier: String,
    client_id: String,
    token_endpoint: String,
    resource: String,
    redirect_uri: String,
}

#[derive(Debug)]
enum FlowOutcome {
    Pending,
    Completed,
    Failed(String),
}

#[derive(Debug)]
struct PendingFlow {
    state: String,
    exchange: Option<ExchangeInputs>,
    executor: BaseCodingAgent,
    server_name: String,
    created_at: Instant,
    outcome: FlowOutcome,
}

static FLOWS: LazyLock<RwLock<HashMap<Uuid, PendingFlow>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/mcp-auth/start", post(start))
        .route("/mcp-auth/callback", get(callback))
        .route("/mcp-auth/complete", post(complete))
        .route("/mcp-auth/status", get(status))
}

/// Resolve the agent's MCP config path, mirroring `test_mcp_servers_route`.
fn agent_config_path(
    executor: BaseCodingAgent,
) -> Result<(std::path::PathBuf, executors::mcp_config::McpConfig), String> {
    let coding_agent = ExecutorConfigs::get_cached()
        .get_coding_agent(&ExecutorProfileId::new(executor))
        .ok_or("Executor not found")?;
    if !coding_agent.supports_mcp() {
        return Err("MCP not supported by this executor".to_string());
    }
    let config_path = coding_agent
        .default_mcp_config_path()
        .ok_or("Could not determine config file path")?;
    Ok((config_path, coding_agent.get_mcp_config()))
}

async fn start(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<McpAuthQuery>,
    Json(payload): Json<McpAuthStartRequest>,
) -> Result<ResponseJson<ApiResponse<McpAuthStartResponse>>, ApiError> {
    let (config_path, mcpc) = match agent_config_path(query.executor) {
        Ok(v) => v,
        Err(message) => return Ok(ResponseJson(ApiResponse::error(&message))),
    };

    let raw_config = read_agent_config(&config_path, &mcpc).await?;
    let servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);
    let Some(entry) = servers.get(&payload.server_name) else {
        return Ok(ResponseJson(ApiResponse::error(&format!(
            "MCP server `{}` is not configured for this agent",
            payload.server_name
        ))));
    };
    let Some(url) = entry
        .get("url")
        .and_then(serde_json::Value::as_str)
        .or_else(|| entry.get("httpUrl").and_then(serde_json::Value::as_str))
    else {
        return Ok(ResponseJson(ApiResponse::error(
            "Only URL-based (http/sse) MCP servers can be authenticated with OAuth",
        )));
    };

    {
        let mut flows = FLOWS.write().await;
        if !prune_flows_and_has_capacity(&mut flows) {
            return Ok(ResponseJson(ApiResponse::error(
                "Too many OAuth flows are already pending; wait for one to finish or expire",
            )));
        }
    }

    let public_base_url = env::var("MCP_OAUTH_PUBLIC_BASE_URL").ok();
    // Local installations should remain one-click: when no canonical public
    // origin is configured, choose the safe loopback flow automatically. An
    // explicitly configured value is still validated and fails closed.
    let use_loopback = payload.loopback || public_base_url.is_none();
    let redirect_uri = if use_loopback {
        // Loopback mode: register a localhost callback so a strict-allowlist
        // authorization server accepts it. This works from anywhere because the
        // flow can be finished by pasting the redirected URL back via
        // `/mcp-auth/complete` — so it deliberately does not require a
        // browser-reachable host and skips the relay guard below.
        let Some(port) = deployment
            .client_info()
            .get_server_addr()
            .map(|addr| addr.port())
        else {
            return Ok(ResponseJson(ApiResponse::error(
                "Could not determine the local server port for a loopback callback",
            )));
        };
        format!("http://localhost:{port}/api/mcp-auth/callback")
    } else {
        match public_callback_uri(public_base_url.as_deref()) {
            Ok(uri) => uri,
            Err(error) => return Ok(ResponseJson(ApiResponse::error(&error))),
        }
    };

    let client = match mcp_oauth::OAuthHttpClient::new(OAUTH_HTTP_TIMEOUT, url) {
        Ok(client) => client,
        Err(error) => return Ok(ResponseJson(ApiResponse::error(&error))),
    };
    let meta = match mcp_oauth::discover(&client, url, payload.www_authenticate.as_deref()).await {
        Ok(meta) => meta,
        Err(e) => return Ok(ResponseJson(ApiResponse::error(&e))),
    };
    let Some(registration_endpoint) = meta.registration_endpoint.as_deref() else {
        return Ok(ResponseJson(ApiResponse::error(&format!(
            "The authorization server does not support dynamic client registration. \
             Authorize manually at {} and paste the token into the server's headers \
             (Authorization: Bearer …) in the edit dialog.",
            meta.authorization_endpoint
        ))));
    };
    let client_id =
        match mcp_oauth::register_client(&client, registration_endpoint, &redirect_uri).await {
            Ok(id) => id,
            Err(e) => {
                return Ok(ResponseJson(ApiResponse::error(&connect_error(
                    e,
                    &redirect_uri,
                ))));
            }
        };

    let pkce = mcp_oauth::Pkce::generate();
    let state = mcp_oauth::generate_state();
    let authorize_url = match mcp_oauth::build_authorize_url(
        &meta.authorization_endpoint,
        &client_id,
        &redirect_uri,
        &pkce.challenge,
        &state,
        &meta.resource,
        &meta.scopes_supported,
    ) {
        Ok(url) => url,
        Err(e) => return Ok(ResponseJson(ApiResponse::error(&e))),
    };

    let flow_id = Uuid::new_v4();
    let flow = PendingFlow {
        state,
        exchange: Some(ExchangeInputs {
            oauth_client: client,
            pkce_verifier: pkce.verifier,
            client_id,
            token_endpoint: meta.token_endpoint,
            resource: meta.resource,
            redirect_uri,
        }),
        executor: query.executor,
        server_name: payload.server_name,
        created_at: Instant::now(),
        outcome: FlowOutcome::Pending,
    };
    {
        let mut flows = FLOWS.write().await;
        if !prune_flows_and_has_capacity(&mut flows) {
            return Ok(ResponseJson(ApiResponse::error(
                "Too many OAuth flows are already pending; wait for one to finish or expire",
            )));
        }
        flows.insert(flow_id, flow);
    }

    Ok(ResponseJson(ApiResponse::success(McpAuthStartResponse {
        flow_id,
        authorize_url,
        loopback: use_loopback,
    })))
}

fn prune_flows_and_has_capacity(flows: &mut HashMap<Uuid, PendingFlow>) -> bool {
    flows.retain(|_, flow| flow.created_at.elapsed() < FLOW_TTL);
    flows.len() < MAX_FLOWS
}

fn public_callback_uri(value: Option<&str>) -> Result<String, String> {
    let raw = value.ok_or_else(|| {
        "Set MCP_OAUTH_PUBLIC_BASE_URL to an HTTPS public URL for automatic callbacks, or use the localhost callback option".to_string()
    })?;
    let mut url = reqwest::Url::parse(raw)
        .map_err(|_| "MCP_OAUTH_PUBLIC_BASE_URL is not a valid URL".to_string())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "MCP_OAUTH_PUBLIC_BASE_URL must be an HTTPS URL without credentials, query, or fragment"
                .to_string(),
        );
    }
    url.set_path("/api/mcp-auth/callback");
    Ok(url.to_string())
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

async fn callback(Query(query): Query<CallbackQuery>) -> Result<Response<String>, ApiError> {
    let Some(state) = query.state.as_deref() else {
        return Ok(simple_html_response(
            StatusCode::BAD_REQUEST,
            "Missing state in callback".to_string(),
        ));
    };

    // Find the flow bound to this state and consume its exchange inputs
    // atomically; whatever happens next, this state can't be redeemed again.
    let (flow_id, exchange, executor, server_name) = {
        let mut flows = FLOWS.write().await;
        prune_flows_and_has_capacity(&mut flows);
        let Some((id, flow)) = flows.iter_mut().find(|(_, f)| f.state == state) else {
            return Ok(simple_html_response(
                StatusCode::BAD_REQUEST,
                "Authorization flow not found or expired".to_string(),
            ));
        };
        let Some(exchange) = flow.exchange.take() else {
            return Ok(simple_html_response(
                StatusCode::BAD_REQUEST,
                "This authorization flow was already completed".to_string(),
            ));
        };
        (*id, exchange, flow.executor, flow.server_name.clone())
    };

    // The failure text can carry attacker-influenced content (OAuth `error` /
    // `error_description` query params, authorization-server response bodies),
    // and the result pages interpolate their message into HTML — escape it.
    // The raw text still reaches the UI via the JSON status endpoint.
    let fail = |message: String| async move {
        if let Some(flow) = FLOWS.write().await.get_mut(&flow_id) {
            flow.outcome = FlowOutcome::Failed(message.clone());
        }
        simple_html_response(StatusCode::BAD_REQUEST, html_escape(&message))
    };

    if let Some(error) = query.error {
        let detail = query.error_description.unwrap_or_default();
        return Ok(fail(
            format!("Authorization failed: {error} {detail}")
                .trim()
                .to_string(),
        )
        .await);
    }
    let Some(code) = query.code else {
        return Ok(fail("Missing authorization code in callback".to_string()).await);
    };

    match exchange_and_store(exchange, executor, &server_name, &code).await {
        Ok(()) => {
            if let Some(flow) = FLOWS.write().await.get_mut(&flow_id) {
                flow.outcome = FlowOutcome::Completed;
            }
            Ok(close_window_response(
                format!(
                    "Connected {}. You can return to the app.",
                    html_escape(&server_name)
                ),
                false,
            ))
        }
        Err(e) => Ok(fail(e).await),
    }
}

/// Redeem an authorization code and write the resulting token onto the
/// server's config entry. Shared by the browser `callback` and the manual
/// `complete` endpoint. The access token never leaves this function except
/// into the agent config file.
async fn exchange_and_store(
    exchange: ExchangeInputs,
    executor: BaseCodingAgent,
    server_name: &str,
    code: &str,
) -> Result<(), String> {
    let access_token = mcp_oauth::exchange_code(
        &exchange.oauth_client,
        &exchange.token_endpoint,
        &exchange.client_id,
        code,
        &exchange.pkce_verifier,
        &exchange.redirect_uri,
        &exchange.resource,
    )
    .await?;
    persist_token(executor, server_name, &access_token).await
}

/// Extract the authorization code and state from a pasted callback URL.
fn parse_pasted_code(input: &str) -> Result<(String, String), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Paste the full redirected URL".to_string());
    }
    let url = reqwest::Url::parse(trimmed)
        .map_err(|_| "Could not read that as a URL or a code".to_string())?;
    let mut code = None;
    let mut state = None;
    let mut oauth_error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => oauth_error = Some(v.into_owned()),
            _ => {}
        }
    }
    if let Some(err) = oauth_error {
        return Err(format!("Authorization failed: {err}"));
    }
    match (code, state) {
        (Some(code), Some(state)) => Ok((code, state)),
        (None, _) => Err("That URL has no `code` parameter".to_string()),
        (_, None) => Err("That URL has no `state` parameter".to_string()),
    }
}

async fn complete(
    Query(_query): Query<McpAuthQuery>,
    Json(payload): Json<McpAuthCompleteRequest>,
) -> Result<ResponseJson<ApiResponse<McpAuthStatusResponse>>, ApiError> {
    let (code, pasted_state) = match parse_pasted_code(&payload.code) {
        Ok(v) => v,
        Err(e) => return Ok(ResponseJson(ApiResponse::error(&e))),
    };

    // Look up the flow by id and consume its exchange inputs atomically, so a
    // pasted code — like a browser callback — can only be redeemed once. If
    // the paste carried a `state`, it must match the flow's (CSRF binding).
    let (exchange, executor, server_name) = {
        let mut flows = FLOWS.write().await;
        prune_flows_and_has_capacity(&mut flows);
        let Some(flow) = flows.get_mut(&payload.flow_id) else {
            return Ok(ResponseJson(ApiResponse::error(
                "Authorization flow not found or expired",
            )));
        };
        if pasted_state != flow.state {
            return Ok(ResponseJson(ApiResponse::error(
                "The pasted URL's state does not match this authorization flow",
            )));
        }
        let Some(exchange) = flow.exchange.take() else {
            return Ok(ResponseJson(ApiResponse::error(
                "This authorization flow was already completed",
            )));
        };
        (exchange, flow.executor, flow.server_name.clone())
    };

    match exchange_and_store(exchange, executor, &server_name, &code).await {
        Ok(()) => {
            if let Some(flow) = FLOWS.write().await.get_mut(&payload.flow_id) {
                flow.outcome = FlowOutcome::Completed;
            }
            Ok(ResponseJson(ApiResponse::success(McpAuthStatusResponse {
                status: McpAuthFlowState::Completed,
                error: None,
            })))
        }
        Err(e) => {
            if let Some(flow) = FLOWS.write().await.get_mut(&payload.flow_id) {
                flow.outcome = FlowOutcome::Failed(e.clone());
            }
            Ok(ResponseJson(ApiResponse::error(&e)))
        }
    }
}

/// Minimal HTML escaping for text interpolated into the OAuth result pages.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Write the token as an `Authorization: Bearer …` header on the server's
/// config entry, through the same read-modify-write path the settings screen
/// uses. The token is deliberately absent from all logging and error strings.
async fn persist_token(
    executor: BaseCodingAgent,
    server_name: &str,
    access_token: &str,
) -> Result<(), String> {
    let (config_path, mcpc) = agent_config_path(executor)?;
    let raw_config = read_agent_config(&config_path, &mcpc)
        .await
        .map_err(|e| format!("failed to read agent config: {e}"))?;
    let mut servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);
    let entry = servers
        .get_mut(server_name)
        .ok_or_else(|| format!("MCP server `{server_name}` is no longer configured"))?;
    let obj = entry
        .as_object_mut()
        .ok_or_else(|| format!("MCP server `{server_name}` entry is not an object"))?;
    let header_key = if executor == BaseCodingAgent::Codex {
        "http_headers"
    } else {
        "headers"
    };
    let headers = obj.entry(header_key).or_insert_with(|| json!({}));
    if !headers.is_object() {
        *headers = json!({});
    }
    headers
        .as_object_mut()
        .expect("headers was just ensured to be an object")
        .insert(
            "Authorization".to_string(),
            json!(format!("Bearer {access_token}")),
        );

    update_mcp_servers_in_config(&config_path, &mcpc, servers)
        .await
        .map_err(|e| format!("failed to write agent config: {e}"))?;
    secure_token_file(&config_path).await
}

#[cfg(unix)]
async fn secure_token_file(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|e| format!("failed to secure agent config: {e}"))
}

#[cfg(not(unix))]
async fn secure_token_file(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Deserialize)]
struct StatusQuery {
    flow_id: Uuid,
}

async fn status(
    Query(query): Query<StatusQuery>,
) -> Result<ResponseJson<ApiResponse<McpAuthStatusResponse>>, ApiError> {
    let flows = FLOWS.read().await;
    let response = match flows.get(&query.flow_id) {
        Some(flow) if flow.created_at.elapsed() < FLOW_TTL => match &flow.outcome {
            FlowOutcome::Pending => McpAuthStatusResponse {
                status: McpAuthFlowState::Pending,
                error: None,
            },
            FlowOutcome::Completed => McpAuthStatusResponse {
                status: McpAuthFlowState::Completed,
                error: None,
            },
            FlowOutcome::Failed(message) => McpAuthStatusResponse {
                status: McpAuthFlowState::Failed,
                error: Some(message.clone()),
            },
        },
        _ => McpAuthStatusResponse {
            status: McpAuthFlowState::Failed,
            error: Some("Authorization flow not found or expired".to_string()),
        },
    };
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_error_adds_guidance_for_public_redirect_rejection() {
        let raw = "client registration failed (HTTP 400): redirect_uri must be a \
                   trusted MCP client callback (Claude, ChatGPT/Codex, Cursor, or localhost)"
            .to_string();
        let enriched = connect_error(raw.clone(), "https://vk.example.com/api/mcp-auth/callback");
        assert!(enriched.starts_with(&raw));
        assert!(enriched.contains("localhost"));
        assert!(enriched.contains("Authorization: Bearer"));
    }

    #[test]
    fn connect_error_untouched_when_already_loopback() {
        // A loopback callback that still gets a redirect complaint shouldn't
        // be told to "open on localhost" — it already is.
        let raw = "client registration failed (HTTP 400): redirect_uri invalid".to_string();
        let enriched = connect_error(raw.clone(), "http://localhost:8080/api/mcp-auth/callback");
        assert_eq!(enriched, raw);
    }

    #[test]
    fn connect_error_guides_public_registration_400_without_remote_body() {
        let raw = "client registration failed (HTTP 400)".to_string();
        let enriched = connect_error(raw.clone(), "https://vk.example.com/api/mcp-auth/callback");
        assert!(enriched.starts_with(&raw));
        assert!(enriched.contains("localhost"));
    }

    #[test]
    fn connect_error_untouched_for_unrelated_failures() {
        let raw = "client registration failed (HTTP 500): internal error".to_string();
        let enriched = connect_error(raw.clone(), "https://vk.example.com/api/mcp-auth/callback");
        assert_eq!(enriched, raw);
    }

    #[test]
    fn parse_pasted_code_rejects_bare_code() {
        assert!(parse_pasted_code("  abc123 ").is_err());
    }

    #[test]
    fn parse_pasted_code_extracts_from_redirect_url() {
        let (code, state) =
            parse_pasted_code("http://localhost:8080/api/mcp-auth/callback?code=xyz789&state=st-1")
                .unwrap();
        assert_eq!(code, "xyz789");
        assert_eq!(state, "st-1");
    }

    #[test]
    fn parse_pasted_code_surfaces_oauth_error() {
        let err = parse_pasted_code(
            "http://localhost:8080/cb?error=access_denied&error_description=nope",
        )
        .unwrap_err();
        assert!(err.contains("access_denied"), "got: {err}");
    }

    #[test]
    fn parse_pasted_code_rejects_empty_and_codeless() {
        assert!(parse_pasted_code("   ").is_err());
        assert!(parse_pasted_code("http://localhost:8080/cb?state=only").is_err());
        assert!(parse_pasted_code("http://localhost:8080/cb?code=only").is_err());
    }

    #[test]
    fn public_callback_uses_only_valid_configured_https_base() {
        assert_eq!(
            public_callback_uri(Some("https://vk.example.com/base/")).unwrap(),
            "https://vk.example.com/api/mcp-auth/callback"
        );
        for invalid in [
            None,
            Some("http://vk.example.com"),
            Some("https://user@vk.example.com"),
            Some("https://vk.example.com?next=https://evil.example"),
        ] {
            assert!(public_callback_uri(invalid).is_err());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn token_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.json");
        tokio::fs::write(&path, "secret").await.unwrap();
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
            .await
            .unwrap();
        secure_token_file(&path).await.unwrap();
        assert_eq!(
            tokio::fs::metadata(path)
                .await
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn flow_capacity_prunes_expired_entries_before_rejecting() {
        let flow = |created_at| PendingFlow {
            state: "state".to_string(),
            exchange: None,
            executor: BaseCodingAgent::ClaudeCode,
            server_name: "server".to_string(),
            created_at,
            outcome: FlowOutcome::Pending,
        };
        let mut flows = HashMap::new();
        for index in 0..MAX_FLOWS {
            flows.insert(Uuid::from_u128(index as u128), flow(Instant::now()));
        }
        assert!(!prune_flows_and_has_capacity(&mut flows));
        flows.insert(
            Uuid::from_u128(0),
            flow(Instant::now() - FLOW_TTL - Duration::from_secs(1)),
        );
        assert!(prune_flows_and_has_capacity(&mut flows));
        assert_eq!(flows.len(), MAX_FLOWS - 1);
    }
}
