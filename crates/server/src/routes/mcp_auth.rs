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
    sync::LazyLock,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::HOST},
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
use relay_client::RELAY_HEADER;
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

/// Per-request bound for every outbound OAuth call (discovery, DCR, code
/// exchange) so a stalled endpoint can't hang `start` or `callback` — the
/// same discipline as the probe's `MCP_TEST_TIMEOUT`.
const OAUTH_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

fn oauth_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(OAUTH_HTTP_TIMEOUT)
        .build()
        // Builder failure here means TLS init failed; a client without the
        // timeout still beats returning 500 for every Connect attempt.
        .unwrap_or_default()
}

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
        lower.contains("redirect_uri") || lower.contains("redirect uri")
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
    /// automatically; otherwise the user pastes the redirected URL/code back
    /// via `/mcp-auth/complete`.
    #[serde(default)]
    pub loopback: bool,
}

#[derive(TS, Debug, Serialize)]
pub struct McpAuthStartResponse {
    pub flow_id: Uuid,
    pub authorize_url: String,
    /// True when this flow used a loopback callback, so the frontend knows to
    /// offer manual code entry if the popup can't reach it.
    pub loopback: bool,
}

#[derive(TS, Debug, Deserialize)]
pub struct McpAuthCompleteRequest {
    pub flow_id: Uuid,
    /// Either the bare authorization `code`, or the full redirect URL the
    /// browser landed on (`http://localhost:…/callback?code=…&state=…`) — the
    /// user copies whichever is easier.
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
    headers: HeaderMap,
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

    let redirect_uri = if payload.loopback {
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
        // Relay-proxied requests reach us with a loopback Host the user's
        // browser cannot be redirected back to, so an auto-callback flow could
        // never complete — fail loudly instead of handing out a broken
        // authorize URL. (Loopback mode above is the escape hatch.)
        if headers
            .get(RELAY_HEADER)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.trim() == "1")
        {
            return Ok(ResponseJson(ApiResponse::error(
                "Connect is only available when accessing Vibe Kanban directly \
                 (not through a relayed host). Retry with the localhost-callback \
                 option, open this machine's own UI, or paste a token into the \
                 server's headers in the edit dialog.",
            )));
        }

        // The browser reached us through this host, so it can be redirected
        // back to it. Behind an HTTPS reverse proxy (e.g. the
        // Caddyfile.example setup) the browser-facing scheme/host arrive in
        // X-Forwarded-* headers — using plain `http://{Host}` there would
        // register a redirect URI the AS may reject and the browser can't load.
        let forwarded_header = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.split(',').next())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let scheme = forwarded_header("x-forwarded-proto")
            .filter(|v| v == "https" || v == "http")
            .unwrap_or_else(|| "http".to_string());
        let host = forwarded_header("x-forwarded-host").or_else(|| {
            headers
                .get(HOST)
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });
        let Some(host) = host else {
            return Ok(ResponseJson(ApiResponse::error(
                "Missing Host header; cannot build a redirect URI",
            )));
        };
        format!("{scheme}://{host}/api/mcp-auth/callback")
    };

    let client = oauth_http_client();
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
        flows.retain(|_, f| f.created_at.elapsed() < FLOW_TTL);
        flows.insert(flow_id, flow);
    }

    Ok(ResponseJson(ApiResponse::success(McpAuthStartResponse {
        flow_id,
        authorize_url,
        loopback: payload.loopback,
    })))
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
        flows.retain(|_, f| f.created_at.elapsed() < FLOW_TTL);
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
    let client = oauth_http_client();
    let access_token = mcp_oauth::exchange_code(
        &client,
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

/// Extract the authorization `code` from a manual paste that is either the
/// bare code or the full redirect URL the browser landed on. When a URL
/// carries `error`/`state`, honor them (surface the error, enforce the CSRF
/// binding). Returns `(code, state_if_present)`.
fn parse_pasted_code(input: &str) -> Result<(String, Option<String>), String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Paste the redirected URL or the authorization code".to_string());
    }
    // A bare code (no URL structure) is used as-is.
    if !trimmed.contains("://") && !trimmed.contains("code=") && !trimmed.contains('?') {
        return Ok((trimmed.to_string(), None));
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
    match code {
        Some(code) => Ok((code, state)),
        None => Err("That URL has no `code` parameter".to_string()),
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
        flows.retain(|_, f| f.created_at.elapsed() < FLOW_TTL);
        let Some(flow) = flows.get_mut(&payload.flow_id) else {
            return Ok(ResponseJson(ApiResponse::error(
                "Authorization flow not found or expired",
            )));
        };
        if let Some(state) = &pasted_state
            && state != &flow.state
        {
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
    let headers = obj.entry("headers").or_insert_with(|| json!({}));
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
        .map(|_| ())
        .map_err(|e| format!("failed to write agent config: {e}"))
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
    fn connect_error_untouched_for_unrelated_failures() {
        let raw = "client registration failed (HTTP 500): internal error".to_string();
        let enriched = connect_error(raw.clone(), "https://vk.example.com/api/mcp-auth/callback");
        assert_eq!(enriched, raw);
    }

    #[test]
    fn parse_pasted_code_accepts_bare_code() {
        assert_eq!(
            parse_pasted_code("  abc123 ").unwrap(),
            ("abc123".to_string(), None)
        );
    }

    #[test]
    fn parse_pasted_code_extracts_from_redirect_url() {
        let (code, state) =
            parse_pasted_code("http://localhost:8080/api/mcp-auth/callback?code=xyz789&state=st-1")
                .unwrap();
        assert_eq!(code, "xyz789");
        assert_eq!(state.as_deref(), Some("st-1"));
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
    }
}
