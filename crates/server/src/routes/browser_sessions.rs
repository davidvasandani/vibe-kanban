use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State, ws::Message},
    http::header,
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson, Response},
    routing::{delete, get, post},
};
use db::models::{
    browser_session::{BrowserControlTransition, BrowserSession, CreateBrowserSession},
    workspace::Workspace,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::browser::{
    BrowserSessionWithState,
    arbiter::TransferTarget,
    types::{
        BrowserAction, BrowserActionResult, BrowserControlState, BrowserPageInfo,
        BrowserSessionError, BrowserSessionLiveState, ControlPrincipal,
    },
};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::signed_ws::SignedWsUpgrade};

// ── Request/response payloads ───────────────────────────────────────────

#[derive(Debug, Deserialize, TS)]
pub struct BrowserSessionListQuery {
    pub workspace_id: Uuid,
    #[serde(default)]
    pub include_closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum BrowserPrincipalKind {
    Human,
    Agent,
}

#[derive(Debug, Deserialize, TS)]
pub struct BrowserAcquireRequest {
    #[serde(rename = "as")]
    pub principal: BrowserPrincipalKind,
    pub execution_id: Option<Uuid>,
    #[serde(default)]
    pub take_from_agent: bool,
    #[serde(default)]
    pub force: bool,
    #[ts(type = "number | null")]
    pub expected_generation: Option<u64>,
}

#[derive(Debug, Deserialize, TS)]
pub struct BrowserReleaseRequest {
    #[serde(rename = "as")]
    pub principal: BrowserPrincipalKind,
    pub execution_id: Option<Uuid>,
    #[ts(type = "number | null")]
    pub expected_generation: Option<u64>,
}

#[derive(Debug, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserTransferTargetRequest {
    None,
    Agent { execution_id: Uuid },
}

#[derive(Debug, Deserialize, TS)]
pub struct BrowserTransferRequest {
    #[serde(rename = "as")]
    pub principal: BrowserPrincipalKind,
    pub execution_id: Option<Uuid>,
    #[ts(type = "number")]
    pub expected_generation: u64,
    pub target: BrowserTransferTargetRequest,
}

#[derive(Debug, Deserialize, TS)]
pub struct BrowserActionRequest {
    #[serde(rename = "as")]
    pub principal: BrowserPrincipalKind,
    pub execution_id: Option<Uuid>,
    pub command_id: Uuid,
    #[ts(type = "number | null")]
    pub expected_generation: Option<u64>,
    pub action: BrowserAction,
    /// Agent action tools may auto-acquire an uncontrolled session; they can
    /// never displace a live controller.
    #[serde(default)]
    pub auto_acquire: bool,
}

#[derive(Debug, Deserialize, TS)]
pub struct BrowserCloseQuery {
    #[serde(default)]
    pub force: bool,
}

/// The human principal for REST-initiated operations. REST leases are not
/// bound to a live-view connection (nil connection id), so they release via
/// lease expiry rather than disconnect.
fn rest_human_principal(deployment: &DeploymentImpl) -> ControlPrincipal {
    ControlPrincipal::Human {
        user_id: deployment.user_id().to_string(),
        connection_id: Uuid::nil(),
    }
}

/// Trust model: the local deployment is single-user, and MCP agent tools
/// reach this API as plain local HTTP — there is no cryptographic execution
/// identity, so `as: "agent"` binds to a running execution in the session's
/// workspace on the caller's word. The workspace-membership check is the
/// enforced boundary; per-execution caller authentication is a recorded
/// hardening seam for multi-user deployments (see
/// homelab/specs/vk/57e0-add-shared-human/research.md).
async fn resolve_principal(
    deployment: &DeploymentImpl,
    kind: BrowserPrincipalKind,
    execution_id: Option<Uuid>,
    workspace_id: Uuid,
) -> Result<ControlPrincipal, BrowserSessionError> {
    match kind {
        BrowserPrincipalKind::Human => Ok(rest_human_principal(deployment)),
        BrowserPrincipalKind::Agent => {
            deployment
                .browser_sessions()
                .resolve_agent_principal(workspace_id, execution_id)
                .await
        }
    }
}

// ── REST handlers ───────────────────────────────────────────────────────

pub async fn create_browser_session(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateBrowserSession>,
) -> Result<ResponseJson<ApiResponse<BrowserSessionWithState>>, ApiError> {
    // Workspace must exist; sessions are workspace-scoped by construction.
    Workspace::find_by_id(&deployment.db().pool, payload.workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("workspace not found".to_string()))?;
    let session = deployment
        .browser_sessions()
        .create_session(payload.workspace_id, payload.profile)
        .await?;
    deployment
        .track_if_analytics_allowed(
            "browser_session_created",
            serde_json::json!({ "workspace_id": payload.workspace_id.to_string() }),
        )
        .await;
    Ok(ResponseJson(ApiResponse::success(session)))
}

pub async fn list_browser_sessions(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<BrowserSessionListQuery>,
) -> Result<ResponseJson<ApiResponse<Vec<BrowserSessionWithState>>>, ApiError> {
    let sessions = deployment
        .browser_sessions()
        .list_sessions(query.workspace_id, query.include_closed)
        .await?;
    Ok(ResponseJson(ApiResponse::success(sessions)))
}

pub async fn get_browser_session(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
) -> Result<ResponseJson<ApiResponse<BrowserSessionWithState>>, ApiError> {
    let session = deployment
        .browser_sessions()
        .get_session(session.id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(session)))
}

pub async fn close_browser_session(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
    Query(query): Query<BrowserCloseQuery>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let principal = rest_human_principal(&deployment);
    deployment
        .browser_sessions()
        .close_session(session.id, &principal, query.force)
        .await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn get_browser_control(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
) -> Result<ResponseJson<ApiResponse<BrowserControlState>>, ApiError> {
    let control = deployment
        .browser_sessions()
        .get_control(session.id)
        .await?;
    Ok(ResponseJson(ApiResponse::success(control)))
}

pub async fn acquire_browser_control(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
    Json(payload): Json<BrowserAcquireRequest>,
) -> Result<ResponseJson<ApiResponse<BrowserControlState>>, ApiError> {
    let principal = resolve_principal(
        &deployment,
        payload.principal,
        payload.execution_id,
        session.workspace_id,
    )
    .await?;
    // Agents can never displace a live controller (take/force are human-only
    // affordances), regardless of what the payload claims.
    let (take_from_agent, force) = match payload.principal {
        BrowserPrincipalKind::Human => (payload.take_from_agent, payload.force),
        BrowserPrincipalKind::Agent => (false, false),
    };
    let control = deployment
        .browser_sessions()
        .acquire_control(
            session.id,
            &principal,
            take_from_agent,
            force,
            payload.expected_generation,
        )
        .await?;
    Ok(ResponseJson(ApiResponse::success(control)))
}

pub async fn release_browser_control(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
    Json(payload): Json<BrowserReleaseRequest>,
) -> Result<ResponseJson<ApiResponse<BrowserControlState>>, ApiError> {
    let principal = resolve_principal(
        &deployment,
        payload.principal,
        payload.execution_id,
        session.workspace_id,
    )
    .await?;
    let control = deployment
        .browser_sessions()
        .release_control(session.id, &principal, payload.expected_generation)
        .await?;
    Ok(ResponseJson(ApiResponse::success(control)))
}

pub async fn transfer_browser_control(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
    Json(payload): Json<BrowserTransferRequest>,
) -> Result<ResponseJson<ApiResponse<BrowserControlState>>, ApiError> {
    let principal = resolve_principal(
        &deployment,
        payload.principal,
        payload.execution_id,
        session.workspace_id,
    )
    .await?;
    let target = match payload.target {
        BrowserTransferTargetRequest::None => TransferTarget::None,
        BrowserTransferTargetRequest::Agent { execution_id } => {
            TransferTarget::Agent { execution_id }
        }
    };
    let control = deployment
        .browser_sessions()
        .transfer_control(session.id, &principal, target, payload.expected_generation)
        .await?;
    Ok(ResponseJson(ApiResponse::success(control)))
}

pub async fn execute_browser_action(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
    Json(payload): Json<BrowserActionRequest>,
) -> Result<ResponseJson<ApiResponse<BrowserActionResult>>, ApiError> {
    let principal = resolve_principal(
        &deployment,
        payload.principal,
        payload.execution_id,
        session.workspace_id,
    )
    .await?;
    let result = deployment
        .browser_sessions()
        .execute_action(
            session.id,
            &principal,
            payload.command_id,
            payload.expected_generation,
            payload.action,
            payload.auto_acquire,
        )
        .await?;
    Ok(ResponseJson(ApiResponse::success(result)))
}

pub async fn browser_screenshot(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
) -> Result<Response, ApiError> {
    let png = deployment.browser_sessions().screenshot(session.id).await?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}

pub async fn browser_page_info(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
) -> Result<ResponseJson<ApiResponse<BrowserPageInfo>>, ApiError> {
    let info = deployment.browser_sessions().page_info(session.id).await?;
    Ok(ResponseJson(ApiResponse::success(info)))
}

pub async fn browser_transitions(
    State(deployment): State<DeploymentImpl>,
    Extension(session): Extension<BrowserSession>,
) -> Result<ResponseJson<ApiResponse<Vec<BrowserControlTransition>>>, ApiError> {
    let transitions =
        BrowserControlTransition::find_by_session(&deployment.db().pool, session.id, 100)
            .await
            .map_err(BrowserSessionError::from)?;
    Ok(ResponseJson(ApiResponse::success(transitions)))
}

// ── Live-view WebSocket ─────────────────────────────────────────────────

#[derive(Debug, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserWsServerMessage {
    Ready {
        connection_id: Uuid,
        state: BrowserSessionLiveState,
    },
    State {
        state: BrowserSessionLiveState,
    },
    /// Immediately followed by one binary message carrying the JPEG frame.
    Frame {
        #[ts(type = "number")]
        seq: u64,
        width: u32,
        height: u32,
    },
    CommandResult {
        command_id: Option<Uuid>,
        ok: bool,
        result: Option<BrowserActionResult>,
        control: Option<BrowserControlState>,
        error: Option<BrowserSessionError>,
    },
}

#[derive(Debug, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserWsClientMessage {
    Input {
        command_id: Uuid,
        #[ts(type = "number | null")]
        expected_generation: Option<u64>,
        action: BrowserAction,
    },
    Acquire {
        #[serde(default)]
        take_from_agent: bool,
        #[serde(default)]
        force: bool,
        #[ts(type = "number | null")]
        expected_generation: Option<u64>,
    },
    Release {
        #[ts(type = "number | null")]
        expected_generation: Option<u64>,
    },
    Transfer {
        #[ts(type = "number")]
        expected_generation: u64,
        target: BrowserTransferTargetRequest,
    },
}

pub async fn browser_session_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(session_id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        let service = deployment.browser_sessions().clone();
        let connection_id = Uuid::new_v4();
        let principal = ControlPrincipal::Human {
            user_id: deployment.user_id().to_string(),
            connection_id,
        };

        let (mut state_rx, mut frames) = match (
            service.watch_state(session_id),
            service.subscribe_frames(session_id),
        ) {
            (Ok(state), Ok(frames)) => (state, frames),
            _ => {
                let _ = socket
                    .send(ws_json(&BrowserWsServerMessage::CommandResult {
                        command_id: None,
                        ok: false,
                        result: None,
                        control: None,
                        error: Some(BrowserSessionError::NotFound),
                    }))
                    .await;
                let _ = socket.close().await;
                return;
            }
        };

        let ready = BrowserWsServerMessage::Ready {
            connection_id,
            state: state_rx.borrow_and_update().clone(),
        };
        if socket.send(ws_json(&ready)).await.is_err() {
            return;
        }

        loop {
            tokio::select! {
                changed = state_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let state = state_rx.borrow_and_update().clone();
                    let closed = matches!(
                        state.status,
                        services::services::browser::types::BrowserSessionStatus::Closed
                    );
                    if socket.send(ws_json(&BrowserWsServerMessage::State { state })).await.is_err() {
                        break;
                    }
                    if closed {
                        break;
                    }
                }
                frame = frames.recv() => {
                    match frame {
                        Ok(frame) => {
                            let meta = BrowserWsServerMessage::Frame {
                                seq: frame.seq,
                                width: frame.width,
                                height: frame.height,
                            };
                            if socket.send(ws_json(&meta)).await.is_err() {
                                break;
                            }
                            if socket.send(Message::Binary(frame.data)).await.is_err() {
                                break;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                incoming = socket.recv() => {
                    let msg = match incoming {
                        Ok(Some(Message::Text(text))) => text.to_string(),
                        Ok(Some(Message::Close(_))) | Ok(None) => break,
                        Ok(Some(_)) => continue,
                        Err(_) => break,
                    };
                    let Ok(client_msg) = serde_json::from_str::<BrowserWsClientMessage>(&msg) else {
                        continue;
                    };
                    let response = handle_ws_client_message(
                        &service,
                        session_id,
                        &principal,
                        client_msg,
                    )
                    .await;
                    if socket.send(ws_json(&response)).await.is_err() {
                        break;
                    }
                }
            }
        }

        // Disconnect releases any lease bound to this connection.
        service.release_for_connection(connection_id).await;
        let _ = socket.close().await;
    })
}

async fn handle_ws_client_message(
    service: &services::services::browser::BrowserSessionService,
    session_id: Uuid,
    principal: &ControlPrincipal,
    msg: BrowserWsClientMessage,
) -> BrowserWsServerMessage {
    let (command_id, outcome): (Option<Uuid>, Result<_, BrowserSessionError>) = match msg {
        BrowserWsClientMessage::Input {
            command_id,
            expected_generation,
            action,
        } => {
            let result = service
                .execute_action(
                    session_id,
                    principal,
                    command_id,
                    expected_generation,
                    action,
                    false,
                )
                .await;
            match result {
                Ok(result) => {
                    return BrowserWsServerMessage::CommandResult {
                        command_id: Some(command_id),
                        ok: true,
                        result: Some(result),
                        control: service.get_control(session_id).await.ok(),
                        error: None,
                    };
                }
                Err(e) => (Some(command_id), Err(e)),
            }
        }
        BrowserWsClientMessage::Acquire {
            take_from_agent,
            force,
            expected_generation,
        } => (
            None,
            service
                .acquire_control(
                    session_id,
                    principal,
                    take_from_agent,
                    force,
                    expected_generation,
                )
                .await,
        ),
        BrowserWsClientMessage::Release {
            expected_generation,
        } => (
            None,
            service
                .release_control(session_id, principal, expected_generation)
                .await,
        ),
        BrowserWsClientMessage::Transfer {
            expected_generation,
            target,
        } => {
            let target = match target {
                BrowserTransferTargetRequest::None => TransferTarget::None,
                BrowserTransferTargetRequest::Agent { execution_id } => {
                    TransferTarget::Agent { execution_id }
                }
            };
            (
                None,
                service
                    .transfer_control(session_id, principal, target, expected_generation)
                    .await,
            )
        }
    };
    match outcome {
        Ok(control) => BrowserWsServerMessage::CommandResult {
            command_id,
            ok: true,
            result: None,
            control: Some(control),
            error: None,
        },
        Err(error) => BrowserWsServerMessage::CommandResult {
            command_id,
            ok: false,
            result: None,
            control: service.get_control(session_id).await.ok(),
            error: Some(error),
        },
    }
}

fn ws_json<T: Serialize>(value: &T) -> Message {
    Message::Text(
        serde_json::to_string(value)
            .expect("browser WS message serialization should not fail")
            .into(),
    )
}

// ── Router ──────────────────────────────────────────────────────────────

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let session_router = Router::new()
        .route("/", get(get_browser_session))
        .route("/", delete(close_browser_session))
        .route("/control", get(get_browser_control))
        .route("/control/acquire", post(acquire_browser_control))
        .route("/control/release", post(release_browser_control))
        .route("/control/transfer", post(transfer_browser_control))
        .route("/actions", post(execute_browser_action))
        .route("/screenshot", get(browser_screenshot))
        .route("/page", get(browser_page_info))
        .route("/transitions", get(browser_transitions))
        .route("/ws", get(browser_session_ws))
        .layer(from_fn_with_state(
            deployment.clone(),
            crate::middleware::load_browser_session_middleware,
        ));

    let collection_router = Router::new()
        .route("/", get(list_browser_sessions).post(create_browser_session))
        .nest("/{browser_session_id}", session_router);

    Router::new().nest("/browser-sessions", collection_router)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_client_messages_parse_wire_shapes() {
        let input: BrowserWsClientMessage = serde_json::from_str(
            r#"{"type":"input","command_id":"6e4d5a56-9a2f-4f6e-8b8c-2f6a6f1f4b30","expected_generation":3,"action":{"type":"click","x":10,"y":20,"button":"left"}}"#,
        )
        .unwrap();
        assert!(matches!(
            input,
            BrowserWsClientMessage::Input {
                expected_generation: Some(3),
                action: BrowserAction::Click { .. },
                ..
            }
        ));

        let acquire: BrowserWsClientMessage =
            serde_json::from_str(r#"{"type":"acquire","take_from_agent":true}"#).unwrap();
        assert!(matches!(
            acquire,
            BrowserWsClientMessage::Acquire {
                take_from_agent: true,
                force: false,
                expected_generation: None,
            }
        ));

        let transfer: BrowserWsClientMessage = serde_json::from_str(
            r#"{"type":"transfer","expected_generation":4,"target":{"type":"agent","execution_id":"6e4d5a56-9a2f-4f6e-8b8c-2f6a6f1f4b30"}}"#,
        )
        .unwrap();
        assert!(matches!(
            transfer,
            BrowserWsClientMessage::Transfer {
                expected_generation: 4,
                target: BrowserTransferTargetRequest::Agent { .. },
            }
        ));
    }

    #[test]
    fn action_request_defaults_auto_acquire_off() {
        let req: BrowserActionRequest = serde_json::from_str(
            r#"{"as":"agent","command_id":"6e4d5a56-9a2f-4f6e-8b8c-2f6a6f1f4b30","action":{"type":"navigate","url":"https://example.com"}}"#,
        )
        .unwrap();
        assert!(!req.auto_acquire);
        assert!(matches!(req.principal, BrowserPrincipalKind::Agent));
    }
}
