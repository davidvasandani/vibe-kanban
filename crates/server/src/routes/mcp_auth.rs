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
    extract::Query,
    http::{HeaderMap, StatusCode, header::HOST},
    response::{Json as ResponseJson, Response},
    routing::{get, post},
};
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

#[derive(Debug, Deserialize)]
pub struct McpAuthQuery {
    executor: BaseCodingAgent,
}

#[derive(TS, Debug, Deserialize)]
pub struct McpAuthStartRequest {
    pub server_name: String,
}

#[derive(TS, Debug, Serialize)]
pub struct McpAuthStartResponse {
    pub flow_id: Uuid,
    pub authorize_url: String,
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

    // The browser reached us at this host, so it can be redirected back to it.
    let Some(host) = headers.get(HOST).and_then(|v| v.to_str().ok()) else {
        return Ok(ResponseJson(ApiResponse::error(
            "Missing Host header; cannot build a redirect URI",
        )));
    };
    let redirect_uri = format!("http://{host}/api/mcp-auth/callback");

    let client = reqwest::Client::new();
    let meta = match mcp_oauth::discover(&client, url, None).await {
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
    let client_id = match mcp_oauth::register_client(&client, registration_endpoint, &redirect_uri)
        .await
    {
        Ok(id) => id,
        Err(e) => return Ok(ResponseJson(ApiResponse::error(&e))),
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

    let fail = |message: String| async move {
        if let Some(flow) = FLOWS.write().await.get_mut(&flow_id) {
            flow.outcome = FlowOutcome::Failed(message.clone());
        }
        simple_html_response(StatusCode::BAD_REQUEST, message)
    };

    if let Some(error) = query.error {
        let detail = query.error_description.unwrap_or_default();
        return Ok(fail(format!("Authorization failed: {error} {detail}").trim().to_string()).await);
    }
    let Some(code) = query.code else {
        return Ok(fail("Missing authorization code in callback".to_string()).await);
    };

    let client = reqwest::Client::new();
    let access_token = match mcp_oauth::exchange_code(
        &client,
        &exchange.token_endpoint,
        &exchange.client_id,
        &code,
        &exchange.pkce_verifier,
        &exchange.redirect_uri,
        &exchange.resource,
    )
    .await
    {
        Ok(token) => token,
        Err(e) => return Ok(fail(e).await),
    };

    if let Err(e) = persist_token(executor, &server_name, &access_token).await {
        return Ok(fail(e).await);
    }

    if let Some(flow) = FLOWS.write().await.get_mut(&flow_id) {
        flow.outcome = FlowOutcome::Completed;
    }
    Ok(close_window_response(
        format!("Connected {server_name}. You can return to the app."),
        false,
    ))
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
    let headers = obj
        .entry("headers")
        .or_insert_with(|| json!({}));
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
