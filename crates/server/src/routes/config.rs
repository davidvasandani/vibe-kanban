use std::collections::HashMap;

use api_types::LoginStatus;
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State, ws::Message},
    http,
    response::{IntoResponse, Json as ResponseJson, Response},
    routing::{get, post, put},
};
use deployment::{Deployment, DeploymentError};
use executors::{
    executors::{
        AvailabilityInfo, BaseAgentCapability, BaseCodingAgent, StandardCodingAgentExecutor,
    },
    mcp_config::{McpConfig, read_agent_config, write_agent_config},
    mcp_test::{McpServerTestResult, test_mcp_servers},
    profile::{ExecutorConfigs, ExecutorProfileId},
    shared_mcp_config::{
        SharedMcpProfileWriteOutcome, SharedMcpProfileWriteStatus, SharedMcpReadResponse,
        SharedMcpTestRequest, SharedMcpTestTarget, SharedMcpWriteRequest, SharedMcpWriteResponse,
        SharedMcpWriteStatus, canonical_definition, load_native_snapshots, load_shared_mcp_config,
        plan_servers_for_executor, reconcile_snapshots, validate_server_identifiers,
        validate_write_request,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use services::services::{
    config::{
        Config, ConfigError, SoundFile,
        editor::{EditorConfig, EditorType},
        save_config_to_file,
    },
    container::ContainerService,
    remote_client::RemoteClientError,
};
use tokio::fs;
use ts_rs::TS;
use utils::{assets::config_path, log_msg::LogMsg, response::ApiResponse};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
    runtime::relay_registration,
};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/info", get(get_user_system_info))
        .route("/config", put(update_config))
        .route("/sounds/{sound}", get(get_sound))
        .route("/mcp-config", get(get_mcp_servers).post(update_mcp_servers))
        .route(
            "/mcp-config/shared",
            get(get_shared_mcp_servers).post(update_shared_mcp_servers),
        )
        .route(
            "/mcp-config/shared/test",
            post(test_shared_mcp_servers_route),
        )
        .route("/mcp-config/test", post(test_mcp_servers_route))
        .route("/profiles", get(get_profiles).put(update_profiles))
        .route(
            "/editors/check-availability",
            get(check_editor_availability),
        )
        .route("/agents/check-availability", get(check_agent_availability))
        .route("/agents/preset-options", get(get_agent_preset_options))
        .route(
            "/agents/discovered-options/ws",
            get(stream_executor_discovered_options_ws),
        )
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct Environment {
    pub os_type: String,
    pub os_version: String,
    pub os_architecture: String,
    pub bitness: String,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        let info = os_info::get();
        Environment {
            os_type: info.os_type().to_string(),
            os_version: info.version().to_string(),
            os_architecture: info.architecture().unwrap_or("unknown").to_string(),
            bitness: info.bitness().to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UserSystemInfo {
    pub version: String,
    pub config: Config,
    pub machine_id: String,
    pub login_status: LoginStatus,
    pub remote_auth_degraded: Option<String>,
    #[serde(flatten)]
    pub profiles: ExecutorConfigs,
    pub environment: Environment,
    /// Capabilities supported per executor (e.g., { "CLAUDE_CODE": ["SESSION_FORK"] })
    pub capabilities: HashMap<String, Vec<BaseAgentCapability>>,
    pub shared_api_base: Option<String>,
    pub preview_proxy_port: Option<u16>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub single_user_mode: bool,
    /// Whether the local deployment can accept attachment uploads.
    /// Always true today (filesystem-backed); kept as a capability flag so
    /// the frontend can render a unified "Attachments" status alongside the
    /// remote deployment's `attachments_enabled`.
    pub attachments_enabled: bool,
}

// TODO: update frontend, BE schema has changed, this replaces GET /config and /config/constants
#[axum::debug_handler]
async fn get_user_system_info(
    State(deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<UserSystemInfo>> {
    let config = deployment.config().read().await.clone();
    let login_status = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        deployment.get_login_status(),
    )
    .await
    {
        Ok(status) => status,
        Err(_) => {
            tracing::warn!("timed out determining login status for /api/info");

            let auth_context = deployment.auth_context();
            let cached_profile = auth_context.cached_profile().await;

            match auth_context.get_credentials().await {
                Some(_) => {
                    if auth_context.remote_auth_degraded_slug().await.is_none() {
                        auth_context
                            .set_remote_auth_degraded_slug(
                                RemoteClientError::generic_degraded_slug(),
                            )
                            .await;
                    }

                    deployment
                        .track_if_analytics_allowed(
                            "login_status_timeout",
                            serde_json::json!({
                                "has_cached_profile": cached_profile.is_some(),
                            }),
                        )
                        .await;

                    LoginStatus::LoggedIn {
                        profile: cached_profile,
                    }
                }
                None => {
                    auth_context.clear_profile().await;
                    auth_context.clear_remote_auth_degraded_slug().await;
                    LoginStatus::LoggedOut
                }
            }
        }
    };

    let user_system_info = UserSystemInfo {
        version: option_env!("VK_GIT_SHA").unwrap_or("dev").to_string(),
        config,
        machine_id: deployment.user_id().to_string(),
        login_status,
        remote_auth_degraded: deployment.auth_context().remote_auth_degraded_slug().await,
        profiles: ExecutorConfigs::get_cached(),
        environment: Environment::new(),
        capabilities: {
            let mut caps: HashMap<String, Vec<BaseAgentCapability>> = HashMap::new();
            let profs = ExecutorConfigs::get_cached();
            for key in profs.executors.keys() {
                if let Some(agent) = profs.get_coding_agent(&ExecutorProfileId::new(*key)) {
                    caps.insert(key.to_string(), agent.capabilities());
                }
            }
            caps
        },
        shared_api_base: deployment.remote_info().get_api_base(),
        preview_proxy_port: deployment.client_info().get_preview_proxy_port(),
        single_user_mode: deployment.single_user_mode(),
        attachments_enabled: true,
    };

    ResponseJson(ApiResponse::success(user_system_info))
}

async fn update_config(
    State(deployment): State<DeploymentImpl>,
    Json(new_config): Json<Config>,
) -> ResponseJson<ApiResponse<Config>> {
    let config_path = config_path();

    // Validate git branch prefix
    if !git::is_valid_branch_prefix(&new_config.git_branch_prefix) {
        return ResponseJson(ApiResponse::error(
            "Invalid git branch prefix. Must be a valid git branch name component without slashes.",
        ));
    }

    // Get old config state before updating
    let old_config = deployment.config().read().await.clone();

    match save_config_to_file(&new_config, &config_path).await {
        Ok(_) => {
            let mut config = deployment.config().write().await;
            *config = new_config.clone();
            drop(config);

            // Track config events when fields transition from false → true and run side effects
            handle_config_events(&deployment, &old_config, &new_config).await;

            ResponseJson(ApiResponse::success(new_config))
        }
        Err(e) => ResponseJson(ApiResponse::error(&format!("Failed to save config: {}", e))),
    }
}

/// Track config events when fields transition from false → true
async fn track_config_events(deployment: &DeploymentImpl, old: &Config, new: &Config) {
    let events = [
        (
            !old.disclaimer_acknowledged && new.disclaimer_acknowledged,
            "onboarding_disclaimer_accepted",
            serde_json::json!({}),
        ),
        (
            !old.onboarding_acknowledged && new.onboarding_acknowledged,
            "onboarding_completed",
            serde_json::json!({
                "profile": new.executor_profile,
                "editor": new.editor
            }),
        ),
        (
            !old.analytics_enabled && new.analytics_enabled,
            "analytics_session_start",
            serde_json::json!({}),
        ),
    ];

    for (should_track, event_name, properties) in events {
        if should_track {
            deployment
                .track_if_analytics_allowed(event_name, properties)
                .await;
        }
    }
}

async fn handle_config_events(deployment: &DeploymentImpl, old: &Config, new: &Config) {
    track_config_events(deployment, old, new).await;

    let old_host_nickname = relay_registration::clean_host_nickname(old, deployment.user_id());
    let new_host_nickname = relay_registration::clean_host_nickname(new, deployment.user_id());

    match (old.relay_enabled, new.relay_enabled) {
        (false, true) => relay_registration::spawn_relay(deployment).await,
        (true, false) => relay_registration::stop_relay(deployment).await,
        (true, true) => {
            if old_host_nickname != new_host_nickname {
                relay_registration::spawn_relay(deployment).await;
            }
        }
        (false, false) => (),
    }
}

async fn get_sound(Path(sound): Path<SoundFile>) -> Result<Response, ApiError> {
    let sound = sound.serve().await.map_err(DeploymentError::Other)?;
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("audio/wav"),
        )
        .body(Body::from(sound.data.into_owned()))
        .unwrap();
    Ok(response)
}

#[derive(TS, Debug, Deserialize)]
pub struct McpServerQuery {
    executor: BaseCodingAgent,
}

#[derive(TS, Debug, Serialize, Deserialize)]
pub struct GetMcpServerResponse {
    // servers: HashMap<String, Value>,
    mcp_config: McpConfig,
    config_path: String,
}

#[derive(TS, Debug, Serialize, Deserialize)]
pub struct UpdateMcpServersBody {
    servers: HashMap<String, Value>,
}

#[derive(TS, Debug, Default, Deserialize)]
pub struct TestMcpServersBody {
    /// When present and non-empty, only these servers are tested; otherwise all
    /// of the agent's configured servers are tested.
    #[serde(default)]
    servers: Option<Vec<String>>,
}

#[derive(TS, Debug, Serialize, Deserialize)]
pub struct SharedMcpAssignmentTestResult {
    pub server_name: String,
    pub executor: BaseCodingAgent,
    pub gateway_status: Option<String>,
    pub upstream_status: Option<String>,
    pub result: McpServerTestResult,
}

/// Per-server connectivity timeout for the MCP test endpoint.
const MCP_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
/// Shared gateways can add an upstream hop, and large MCP servers may take
/// longer to return their tool catalog. Keep native probes responsive while
/// giving the full shared-gateway handshake a realistic budget.
const SHARED_MCP_TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

async fn get_mcp_servers(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<McpServerQuery>,
) -> Result<ResponseJson<ApiResponse<GetMcpServerResponse>>, ApiError> {
    let coding_agent = ExecutorConfigs::get_cached()
        .get_coding_agent(&ExecutorProfileId::new(query.executor))
        .ok_or(ConfigError::ValidationError(
            "Executor not found".to_string(),
        ))?;

    if !coding_agent.supports_mcp() {
        return Ok(ResponseJson(ApiResponse::error(
            "MCP not supported by this executor",
        )));
    }

    // Resolve supplied config path or agent default
    let config_path = match coding_agent.default_mcp_config_path() {
        Some(path) => path,
        None => {
            return Ok(ResponseJson(ApiResponse::error(
                "Could not determine config file path",
            )));
        }
    };

    let mut mcpc = coding_agent.get_mcp_config();
    let raw_config = read_agent_config(&config_path, &mcpc).await?;
    let servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);
    mcpc.set_servers(servers);
    Ok(ResponseJson(ApiResponse::success(GetMcpServerResponse {
        mcp_config: mcpc,
        config_path: config_path.to_string_lossy().to_string(),
    })))
}

async fn get_shared_mcp_servers(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<SharedMcpReadResponse>>, ApiError> {
    let mut response = load_shared_mcp_config().await;
    attach_gateway_status(&deployment, &mut response).await;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn attach_gateway_status(deployment: &DeploymentImpl, response: &mut SharedMcpReadResponse) {
    for server in &mut response.servers {
        let Some(url) = server.definition.value.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some(id) = url
            .split("/mcp-gateway/")
            .nth(1)
            .and_then(|tail| tail.split(['/', '?']).next())
        else {
            continue;
        };
        server.gateway_status = db::models::mcp_gateway::McpGatewayConnection::find_bound(
            &deployment.db().pool,
            id,
            deployment.user_id(),
            deployment.user_id(),
        )
        .await
        .ok()
        .flatten()
        .map(|connection| connection.status);
    }
}

async fn update_shared_mcp_servers(
    State(_deployment): State<DeploymentImpl>,
    Json(mut payload): Json<SharedMcpWriteRequest>,
) -> Result<ResponseJson<ApiResponse<SharedMcpWriteResponse>>, ApiError> {
    let snapshots = load_native_snapshots().await;
    if let Err(message) = hydrate_gateway_capabilities(&mut payload, &snapshots) {
        return Ok(ResponseJson(ApiResponse::error(&message)));
    }
    if let Err(message) = validate_write_request(&payload) {
        return Ok(ResponseJson(ApiResponse::error(&message)));
    }

    let mut outcomes = Vec::new();
    let mut any_success = false;
    let mut any_failed = false;

    for snapshot in &snapshots {
        let Ok((planned_servers, affected_servers)) =
            plan_servers_for_executor(snapshot.profile.executor, &snapshot.servers, &payload)
        else {
            // `validate_write_request` catches compatibility before writes.
            continue;
        };

        if affected_servers.is_empty() {
            outcomes.push(SharedMcpProfileWriteOutcome {
                executor: snapshot.profile.executor,
                config_path: snapshot.profile.config_path.clone(),
                status: SharedMcpProfileWriteStatus::Skipped,
                affected_servers,
                message: Some("No MCP server changes for this profile".to_string()),
                error: None,
            });
            continue;
        }

        let Some(config_path) = snapshot.config_path.as_ref() else {
            any_failed = true;
            outcomes.push(SharedMcpProfileWriteOutcome {
                executor: snapshot.profile.executor,
                config_path: None,
                status: SharedMcpProfileWriteStatus::Failed,
                affected_servers,
                message: None,
                error: Some("Could not determine config file path".to_string()),
            });
            continue;
        };

        match update_mcp_servers_in_config(config_path, &snapshot.mcp_config, planned_servers).await
        {
            Ok(message) => {
                any_success = true;
                outcomes.push(SharedMcpProfileWriteOutcome {
                    executor: snapshot.profile.executor,
                    config_path: snapshot.profile.config_path.clone(),
                    status: SharedMcpProfileWriteStatus::Success,
                    affected_servers,
                    message: Some(message),
                    error: None,
                });
            }
            Err(e) => {
                any_failed = true;
                outcomes.push(SharedMcpProfileWriteOutcome {
                    executor: snapshot.profile.executor,
                    config_path: snapshot.profile.config_path.clone(),
                    status: SharedMcpProfileWriteStatus::Failed,
                    affected_servers,
                    message: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let mut fresh = reconcile_snapshots(load_native_snapshots().await);
    attach_gateway_status(&_deployment, &mut fresh).await;
    let status = if any_failed && any_success {
        SharedMcpWriteStatus::PartialFailure
    } else if any_failed {
        SharedMcpWriteStatus::Failed
    } else {
        SharedMcpWriteStatus::Success
    };

    Ok(ResponseJson(ApiResponse::success(SharedMcpWriteResponse {
        status,
        outcomes,
        servers: fresh.servers,
        conflicts: fresh.conflicts,
    })))
}

fn hydrate_gateway_capabilities(
    payload: &mut SharedMcpWriteRequest,
    snapshots: &[executors::shared_mcp_config::NativeProfileSnapshot],
) -> Result<(), String> {
    for server in &mut payload.servers {
        let placeholder = server
            .definition
            .value
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| {
                headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            })
            .is_some_and(|(_, value)| value.as_str() == Some("Bearer [REDACTED]"));
        if !placeholder {
            continue;
        }
        let Some(url) = server.definition.value.get("url").and_then(Value::as_str) else {
            return Err(
                "Redacted gateway credentials cannot be saved without a gateway URL".into(),
            );
        };
        if !url.contains("/mcp-gateway/") {
            return Err(
                "Redacted gateway credentials cannot be reused for a direct MCP server; remove the Authorization header or reconnect"
                    .into(),
            );
        }
        let capability = snapshots
            .iter()
            .flat_map(|snapshot| snapshot.servers.values())
            .find_map(|entry| {
                let definition = canonical_definition(entry);
                (definition.value.get("url").and_then(Value::as_str) == Some(url))
                    .then(|| {
                        definition
                            .value
                            .get("headers")
                            .and_then(Value::as_object)
                            .and_then(|headers| {
                                headers
                                    .iter()
                                    .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                            })
                            .map(|(_, value)| value.clone())
                    })
                    .flatten()
            });
        let Some(capability) = capability else {
            return Err("Shared gateway capability is unavailable; reconnect the server".into());
        };
        if let Some(headers) = server
            .definition
            .value
            .get_mut("headers")
            .and_then(Value::as_object_mut)
            && let Some((_, value)) = headers
                .iter_mut()
                .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        {
            *value = capability;
        }
    }
    Ok(())
}

async fn update_mcp_servers(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<McpServerQuery>,
    Json(payload): Json<UpdateMcpServersBody>,
) -> Result<ResponseJson<ApiResponse<String>>, ApiError> {
    let profiles = ExecutorConfigs::get_cached();
    let agent = profiles
        .get_coding_agent(&ExecutorProfileId::new(query.executor))
        .ok_or(ConfigError::ValidationError(
            "Executor not found".to_string(),
        ))?;

    if !agent.supports_mcp() {
        return Ok(ResponseJson(ApiResponse::error(
            "This executor does not support MCP servers",
        )));
    }

    // Resolve supplied config path or agent default
    let config_path = match agent.default_mcp_config_path() {
        Some(path) => path.to_path_buf(),
        None => {
            return Ok(ResponseJson(ApiResponse::error(
                "Could not determine config file path",
            )));
        }
    };

    let mcpc = agent.get_mcp_config();
    match update_mcp_servers_in_config(&config_path, &mcpc, payload.servers).await {
        Ok(message) => Ok(ResponseJson(ApiResponse::success(message))),
        Err(e) => Ok(ResponseJson(ApiResponse::error(&format!(
            "Failed to update MCP servers: {}",
            e
        )))),
    }
}

/// Probe the MCP servers configured on disk for the given agent and report,
/// per server, whether Vibe Kanban can connect and list tools. Read-only: never
/// modifies the config file.
///
/// Note on stdio probes: testing an stdio server spawns its configured command
/// (the exact command the agent itself would run). This route lives in the
/// `relay_signed_routes` group — the same auth boundary as the `/mcp-config`
/// write endpoint and the agent-execution endpoints — so any caller able to
/// reach it can already both write that command and start an agent that runs it.
/// Commands are read only from the on-disk config (never from the request body),
/// so this does not widen the existing trust boundary. Each probe is bounded by
/// `MCP_TEST_TIMEOUT`.
async fn test_mcp_servers_route(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<McpServerQuery>,
    body: axum::body::Bytes,
) -> Result<ResponseJson<ApiResponse<Vec<McpServerTestResult>>>, ApiError> {
    // The body is optional: an empty body means "test all of the agent's
    // servers". Use `Bytes` rather than `Json<…>` so a body-less request isn't
    // rejected by the extractor before we can apply that default.
    let body: TestMcpServersBody = if body.is_empty() {
        TestMcpServersBody::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| ConfigError::ValidationError(format!("invalid request body: {e}")))?
    };
    let coding_agent = ExecutorConfigs::get_cached()
        .get_coding_agent(&ExecutorProfileId::new(query.executor))
        .ok_or(ConfigError::ValidationError(
            "Executor not found".to_string(),
        ))?;

    if !coding_agent.supports_mcp() {
        return Ok(ResponseJson(ApiResponse::error(
            "MCP not supported by this executor",
        )));
    }

    let config_path = match coding_agent.default_mcp_config_path() {
        Some(path) => path,
        None => {
            return Ok(ResponseJson(ApiResponse::error(
                "Could not determine config file path",
            )));
        }
    };

    let mcpc = coding_agent.get_mcp_config();
    let raw_config = read_agent_config(&config_path, &mcpc).await?;
    let mut servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);

    // Optionally restrict to a named subset of servers.
    if let Some(names) = &body.servers
        && !names.is_empty()
    {
        servers.retain(|name, _| names.contains(name));
    }

    let results = test_mcp_servers(servers, MCP_TEST_TIMEOUT).await;
    Ok(ResponseJson(ApiResponse::success(results)))
}

async fn test_shared_mcp_servers_route(
    State(_deployment): State<DeploymentImpl>,
    body: axum::body::Bytes,
) -> Result<ResponseJson<ApiResponse<Vec<SharedMcpAssignmentTestResult>>>, ApiError> {
    let body: SharedMcpTestRequest = if body.is_empty() {
        SharedMcpTestRequest::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| ConfigError::ValidationError(format!("invalid request body: {e}")))?
    };
    let mut targets = body.targets;

    if targets.is_empty() {
        let shared = load_shared_mcp_config().await;
        targets = shared
            .servers
            .iter()
            .flat_map(|server| {
                server
                    .assignments
                    .iter()
                    .map(|assignment| SharedMcpTestTarget {
                        server_name: server.name.clone(),
                        executor: assignment.executor,
                    })
            })
            .collect();
    }

    let mut by_executor: HashMap<BaseCodingAgent, Vec<String>> = HashMap::new();
    for target in targets {
        by_executor
            .entry(target.executor)
            .or_default()
            .push(target.server_name);
    }

    let mut all_results = Vec::new();
    for (executor, server_names) in by_executor {
        let coding_agent = ExecutorConfigs::get_cached()
            .get_coding_agent(&ExecutorProfileId::new(executor))
            .ok_or(ConfigError::ValidationError(
                "Executor not found".to_string(),
            ))?;

        if !coding_agent.supports_mcp() {
            continue;
        }

        let Some(config_path) = coding_agent.default_mcp_config_path() else {
            continue;
        };
        let mcpc = coding_agent.get_mcp_config();
        let raw_config = read_agent_config(&config_path, &mcpc).await?;
        let mut servers = get_mcp_servers_from_config_path(&raw_config, &mcpc.servers_path);
        servers.retain(|name, _| server_names.contains(name));
        let (gateway_servers, native_servers): (HashMap<_, _>, HashMap<_, _>) =
            servers.into_iter().partition(|(_, entry)| {
                entry
                    .get("url")
                    .or_else(|| entry.get("httpUrl"))
                    .and_then(Value::as_str)
                    .is_some_and(|url| url.contains("/mcp-gateway/"))
            });
        let gateway_names = gateway_servers
            .keys()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let (gateway_results, native_results) = tokio::join!(
            test_mcp_servers(gateway_servers, SHARED_MCP_TEST_TIMEOUT),
            test_mcp_servers(native_servers, MCP_TEST_TIMEOUT)
        );

        for result in gateway_results.into_iter().chain(native_results) {
            let gateway_managed = gateway_names.contains(&result.name);
            let disconnected = result
                .error
                .as_deref()
                .is_some_and(|error| error.contains("disconnected"));
            all_results.push(SharedMcpAssignmentTestResult {
                server_name: result.name.clone(),
                executor,
                gateway_status: gateway_managed
                    .then(|| if disconnected { "disconnected" } else { "ok" }.to_string()),
                upstream_status: gateway_managed.then(|| match &result.status {
                    executors::mcp_test::McpServerTestStatus::Ok => "ok".to_string(),
                    executors::mcp_test::McpServerTestStatus::AuthRequired => {
                        "auth_required".to_string()
                    }
                    executors::mcp_test::McpServerTestStatus::Unsupported => {
                        "unsupported".to_string()
                    }
                    executors::mcp_test::McpServerTestStatus::Failed => "failed".to_string(),
                }),
                result,
            });
        }
    }

    all_results.sort_by(|a, b| {
        a.server_name
            .cmp(&b.server_name)
            .then_with(|| a.executor.to_string().cmp(&b.executor.to_string()))
    });
    Ok(ResponseJson(ApiResponse::success(all_results)))
}

pub(crate) async fn update_mcp_servers_in_config(
    config_path: &std::path::Path,
    mcpc: &McpConfig,
    new_servers: HashMap<String, Value>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    validate_server_identifiers(new_servers.keys().map(String::as_str))?;

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    // Read existing config (JSON or TOML depending on agent)
    let mut config = read_agent_config(config_path, mcpc).await?;

    // Get the current server count for comparison
    let old_servers = get_mcp_servers_from_config_path(&config, &mcpc.servers_path).len();

    // Set the MCP servers using the correct attribute path
    set_mcp_servers_in_config_path(&mut config, &mcpc.servers_path, &new_servers)?;

    // Write the updated config back to file (JSON or TOML depending on agent)
    write_agent_config(config_path, mcpc, &config).await?;

    let new_count = new_servers.len();
    let message = match (old_servers, new_count) {
        (0, 0) => "No MCP servers configured".to_string(),
        (0, n) => format!("Added {} MCP server(s)", n),
        (old, new) if old == new => format!("Updated MCP server configuration ({} server(s))", new),
        (old, new) => format!(
            "Updated MCP server configuration (was {}, now {})",
            old, new
        ),
    };

    Ok(message)
}

/// Helper function to get MCP servers from config using a path
pub(crate) fn get_mcp_servers_from_config_path(
    raw_config: &Value,
    path: &[String],
) -> HashMap<String, Value> {
    let mut current = raw_config;
    for part in path {
        current = match current.get(part) {
            Some(val) => val,
            None => return HashMap::new(),
        };
    }
    // Extract the servers object
    match current.as_object() {
        Some(servers) => servers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        None => HashMap::new(),
    }
}

/// Helper function to set MCP servers in config using a path
fn set_mcp_servers_in_config_path(
    raw_config: &mut Value,
    path: &[String],
    servers: &HashMap<String, Value>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Ensure config is an object
    if !raw_config.is_object() {
        *raw_config = serde_json::json!({});
    }

    // An empty path leaves no attribute to set; treat it as a misconfiguration
    // rather than panicking on the slice/`last()` operations below.
    let (final_attr, parents) = path
        .split_last()
        .ok_or("MCP servers_path must not be empty")?;

    let mut current = raw_config;
    // Navigate/create the nested structure (all parts except the last)
    for part in parents {
        if current.get(part).is_none() {
            current
                .as_object_mut()
                .ok_or("config node is not a JSON object")?
                .insert(part.to_string(), serde_json::json!({}));
        }
        current = current
            .get_mut(part)
            .ok_or("failed to navigate config node")?;
        if !current.is_object() {
            *current = serde_json::json!({});
        }
    }

    // Set the final attribute
    current
        .as_object_mut()
        .ok_or("config node is not a JSON object")?
        .insert(final_attr.to_string(), serde_json::to_value(servers)?);

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfilesContent {
    pub content: String,
    pub path: String,
}

async fn get_profiles(
    State(_deployment): State<DeploymentImpl>,
) -> ResponseJson<ApiResponse<ProfilesContent>> {
    let profiles_path = utils::assets::profiles_path();

    // Use cached data to ensure consistency with runtime and PUT updates
    let profiles = ExecutorConfigs::get_cached();

    let content = serde_json::to_string_pretty(&profiles).unwrap_or_else(|e| {
        tracing::error!("Failed to serialize profiles to JSON: {}", e);
        serde_json::to_string_pretty(&ExecutorConfigs::from_defaults())
            .unwrap_or_else(|_| "{}".to_string())
    });

    ResponseJson(ApiResponse::success(ProfilesContent {
        content,
        path: profiles_path.display().to_string(),
    }))
}

async fn update_profiles(
    State(_deployment): State<DeploymentImpl>,
    body: String,
) -> ResponseJson<ApiResponse<String>> {
    // Try to parse as ExecutorProfileConfigs format
    match serde_json::from_str::<ExecutorConfigs>(&body) {
        Ok(executor_profiles) => {
            // Save the profiles to file
            match executor_profiles.save_overrides() {
                Ok(_) => {
                    tracing::info!("Executor profiles saved successfully");
                    // Reload the cached profiles
                    ExecutorConfigs::reload();
                    ResponseJson(ApiResponse::success(
                        "Executor profiles updated successfully".to_string(),
                    ))
                }
                Err(e) => {
                    tracing::error!("Failed to save executor profiles: {}", e);
                    ResponseJson(ApiResponse::error(&format!(
                        "Failed to save executor profiles: {}",
                        e
                    )))
                }
            }
        }
        Err(e) => ResponseJson(ApiResponse::error(&format!(
            "Invalid executor profiles format: {}",
            e
        ))),
    }
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckEditorAvailabilityQuery {
    editor_type: EditorType,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckEditorAvailabilityResponse {
    available: bool,
}

async fn check_editor_availability(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<CheckEditorAvailabilityQuery>,
) -> ResponseJson<ApiResponse<CheckEditorAvailabilityResponse>> {
    // Construct a minimal EditorConfig for checking
    let editor_config = EditorConfig::new(
        query.editor_type,
        None,  // custom_command
        None,  // remote_ssh_host
        None,  // remote_ssh_user
        false, // auto_install_extension
    );

    let available = editor_config.check_availability().await;
    ResponseJson(ApiResponse::success(CheckEditorAvailabilityResponse {
        available,
    }))
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct CheckAgentAvailabilityQuery {
    executor: BaseCodingAgent,
}

async fn check_agent_availability(
    State(_deployment): State<DeploymentImpl>,
    Query(query): Query<CheckAgentAvailabilityQuery>,
) -> ResponseJson<ApiResponse<AvailabilityInfo>> {
    let profiles = ExecutorConfigs::get_cached();
    let profile_id = ExecutorProfileId::new(query.executor);

    let info = match profiles.get_coding_agent(&profile_id) {
        Some(agent) => agent.get_availability_info(),
        None => AvailabilityInfo::NotFound,
    };

    ResponseJson(ApiResponse::success(info))
}

#[derive(Debug, Deserialize, TS)]
pub struct AgentPresetOptionsQuery {
    pub executor: BaseCodingAgent,
    pub variant: Option<String>,
}

async fn get_agent_preset_options(
    Query(query): Query<AgentPresetOptionsQuery>,
) -> ResponseJson<ApiResponse<executors::profile::ExecutorConfig>> {
    let profiles = ExecutorConfigs::get_cached();
    let profile_id = if let Some(variant) = query.variant {
        ExecutorProfileId::with_variant(query.executor, variant)
    } else {
        ExecutorProfileId::new(query.executor)
    };

    let options = match profiles.get_coding_agent(&profile_id) {
        Some(agent) => agent.get_preset_options(),
        None => {
            // Return a default config if not found
            executors::profile::ExecutorConfig::new(query.executor)
        }
    };

    ResponseJson(ApiResponse::success(options))
}

#[derive(Debug, Deserialize)]
pub struct ExecutorDiscoveredOptionsStreamQuery {
    executor: BaseCodingAgent,
    #[serde(default)]
    session_id: Option<Uuid>,
    #[serde(default)]
    workspace_id: Option<Uuid>,
    #[serde(default)]
    repo_id: Option<Uuid>,
}

pub async fn stream_executor_discovered_options_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ExecutorDiscoveredOptionsStreamQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_executor_discovered_options_ws(socket, deployment, query).await {
            tracing::warn!("discovered options WS closed: {}", e);
        }
    })
}

async fn handle_executor_discovered_options_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    query: ExecutorDiscoveredOptionsStreamQuery,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;

    match deployment
        .container()
        .discover_executor_options(
            ExecutorProfileId::new(query.executor),
            query.session_id,
            query.workspace_id,
            query.repo_id,
        )
        .await
    {
        Ok(Some(mut stream)) => {
            if let Some(patch) = stream.next().await {
                let _ = socket
                    .send(LogMsg::JsonPatch(patch).to_ws_message_unchecked())
                    .await;
            }

            let _ = socket.send(LogMsg::Ready.to_ws_message_unchecked()).await;

            loop {
                tokio::select! {
                    patch = stream.next() => {
                        let Some(patch) = patch else {
                            break;
                        };
                        if socket
                            .send(LogMsg::JsonPatch(patch).to_ws_message_unchecked())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    inbound = socket.recv() => {
                        match inbound {
                            Ok(Some(Message::Close(_))) => break,
                            Ok(Some(_)) => {}
                            Ok(None) => break,
                            Err(_) => break,
                        }
                    }
                }
            }
        }
        Ok(None) => {
            let _ = socket.send(LogMsg::Ready.to_ws_message_unchecked()).await;
        }
        Err(e) => {
            tracing::warn!("Failed to start discovered options stream: {}", e);
        }
    }

    let _ = socket
        .send(LogMsg::Finished.to_ws_message_unchecked())
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn servers(name: &str) -> HashMap<String, Value> {
        HashMap::from([(name.to_string(), serde_json::json!({ "command": "x" }))])
    }

    #[test]
    fn sets_nested_attribute_creating_missing_objects() {
        let mut config = serde_json::json!({});
        let path = vec!["a".to_string(), "b".to_string(), "mcpServers".to_string()];
        set_mcp_servers_in_config_path(&mut config, &path, &servers("srv")).unwrap();
        assert_eq!(
            config["a"]["b"]["mcpServers"]["srv"]["command"],
            serde_json::json!("x")
        );
    }

    #[test]
    fn overwrites_non_object_nodes_in_path() {
        let mut config = serde_json::json!({ "a": 5 });
        let path = vec!["a".to_string(), "mcpServers".to_string()];
        set_mcp_servers_in_config_path(&mut config, &path, &servers("srv")).unwrap();
        assert!(config["a"]["mcpServers"]["srv"].is_object());
    }

    #[test]
    fn empty_path_is_an_error_not_a_panic() {
        let mut config = serde_json::json!({});
        let err = set_mcp_servers_in_config_path(&mut config, &[], &servers("srv"));
        assert!(err.is_err());
    }
}
