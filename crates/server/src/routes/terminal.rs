use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use axum::{
    Router,
    extract::{Query, State, ws::Message},
    response::IntoResponse,
    routing::get,
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use chrono::Utc;
use cluster_protocol::{
    PROTOCOL_VERSION, RequestAuthority, TerminalClose, TerminalCreateRequest, TerminalInput,
    TerminalResize,
};
use db::models::{
    workspace::{Workspace, WorkspacePlacement},
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
};

#[derive(Debug, Deserialize)]
struct TerminalQuery {
    pub workspace_id: Uuid,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalCommand {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TerminalMessage {
    Output { data: String },
    Error { message: String },
}

async fn terminal_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<TerminalQuery>,
) -> Result<axum::response::Response, ApiError> {
    let attempt = Workspace::find_by_id(&deployment.db().pool, query.workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Attempt not found".to_string()))?;

    let container_ref = attempt
        .container_ref
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Attempt has no workspace directory".to_string()))?;

    let base_dir = PathBuf::from(&container_ref);
    if !base_dir.exists() {
        return Err(ApiError::BadRequest(
            "Workspace directory does not exist".to_string(),
        ));
    }

    let mut working_dir = base_dir.clone();
    match WorkspaceRepo::find_repos_for_workspace(&deployment.db().pool, query.workspace_id).await {
        Ok(repos) if repos.len() == 1 => {
            let repo_dir = base_dir.join(&repos[0].name);
            if repo_dir.exists() {
                working_dir = repo_dir;
            }
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to resolve repos for workspace {}: {}",
                attempt.id,
                e
            );
        }
    }

    let mut environment = deployment.container().resolve_org_env_vars(&attempt).await;

    let placement = WorkspacePlacement::find(&deployment.db().pool, attempt.id).await?;
    if let Some(worker_node_id) = terminal_worker_id(placement) {
        let client = deployment.worker_client().cloned().ok_or_else(|| {
            ApiError::BadRequest("Cluster worker client is not configured".into())
        })?;
        let coordinator_id = deployment.cluster_config().coordinator_id.ok_or_else(|| {
            ApiError::BadRequest("Cluster coordinator identity is missing".into())
        })?;
        let request = TerminalCreateRequest {
            authority: terminal_authority(coordinator_id, worker_node_id, attempt.id),
            workspace_id: attempt.id,
            workspace_path: base_dir.to_string_lossy().into_owned(),
            working_directory: working_dir.to_string_lossy().into_owned(),
            environment: environment.into_iter().collect::<BTreeMap<_, _>>(),
            cols: query.cols,
            rows: query.rows,
        };
        let terminal = client
            .create_terminal(worker_node_id, &request)
            .await
            .map_err(|error| ApiError::BadRequest(error.to_string()))?;
        return Ok(ws
            .on_upgrade(move |socket| {
                handle_remote_terminal_ws(
                    socket,
                    client,
                    coordinator_id,
                    worker_node_id,
                    attempt.id,
                    terminal.terminal_id,
                )
            })
            .into_response());
    }

    let inherited_path = environment
        .get("PATH")
        .map(std::ffi::OsString::from)
        .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default());
    if let Some(path) = utils::shell::append_cli_tools_to_path(&inherited_path) {
        environment.insert("PATH".into(), path.to_string_lossy().into_owned());
    }

    Ok(ws
        .on_upgrade(move |socket| {
            handle_terminal_ws(
                socket,
                deployment,
                working_dir,
                environment,
                query.cols,
                query.rows,
            )
        })
        .into_response())
}

fn terminal_worker_id(placement: Option<WorkspacePlacement>) -> Option<Uuid> {
    placement.and_then(|placement| placement.worker_node_id)
}

fn terminal_authority(
    coordinator_id: Uuid,
    worker_node_id: Uuid,
    correlation_id: Uuid,
) -> RequestAuthority {
    RequestAuthority {
        protocol_version: PROTOCOL_VERSION,
        coordinator_id,
        worker_node_id,
        correlation_id,
        issued_at: Utc::now(),
        nonce: Uuid::new_v4().to_string(),
    }
}

async fn handle_remote_terminal_ws(
    mut socket: MaybeSignedWebSocket,
    client: services::services::cluster::WorkerClient,
    coordinator_id: Uuid,
    worker_node_id: Uuid,
    workspace_id: Uuid,
    terminal_id: Uuid,
) {
    let mut poll = tokio::time::interval(Duration::from_millis(50));
    loop {
        tokio::select! {
            _ = poll.tick() => {
                match client.terminal_output(worker_node_id, terminal_id).await {
                    Ok(batch) => {
                        for data in batch.chunks_base64 {
                            let message = TerminalMessage::Output { data };
                            if socket.send(Message::Text(serde_json::to_string(&message).unwrap_or_default().into())).await.is_err() {
                                break;
                            }
                        }
                        if batch.closed { break; }
                    }
                    Err(error) => {
                        tracing::warn!(%terminal_id, "remote terminal output failed: {error}");
                        let _ = send_error(&mut socket, &error.to_string()).await;
                        break;
                    }
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Text(text))) => {
                        if let Ok(command) = serde_json::from_str::<TerminalCommand>(text.as_str()) {
                            let result = match command {
                                TerminalCommand::Input { data } => client.terminal_input(worker_node_id, &TerminalInput {
                                    authority: terminal_authority(coordinator_id, worker_node_id, workspace_id),
                                    terminal_id,
                                    data_base64: data,
                                }).await,
                                TerminalCommand::Resize { cols, rows } => client.terminal_resize(worker_node_id, &TerminalResize {
                                    authority: terminal_authority(coordinator_id, worker_node_id, workspace_id),
                                    terminal_id,
                                    cols,
                                    rows,
                                }).await,
                            };
                            if let Err(error) = result {
                                tracing::warn!(%terminal_id, "remote terminal command failed: {error}");
                                break;
                            }
                        }
                    }
                    Ok(Some(Message::Close(_))) | Ok(None) => break,
                    Ok(Some(_)) => {}
                    Err(error) => {
                        tracing::warn!(%terminal_id, "remote terminal websocket failed: {error}");
                        break;
                    }
                }
            }
        }
    }
    let _ = client
        .close_terminal(
            worker_node_id,
            &TerminalClose {
                authority: terminal_authority(coordinator_id, worker_node_id, workspace_id),
                terminal_id,
            },
        )
        .await;
}

async fn handle_terminal_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    working_dir: PathBuf,
    environment: std::collections::HashMap<String, String>,
    cols: u16,
    rows: u16,
) {
    let (session_id, mut output_rx) = match deployment
        .pty()
        .create_session(working_dir, environment, cols, rows)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to create PTY session: {}", e);
            let _ = send_error(&mut socket, &e.to_string()).await;
            return;
        }
    };

    let pty_service = deployment.pty().clone();
    let session_id_for_input = session_id;

    loop {
        tokio::select! {
            maybe_output = output_rx.recv() => {
                let Some(data) = maybe_output else {
                    break;
                };

                let msg = TerminalMessage::Output {
                    data: BASE64.encode(&data),
                };
                let json = match serde_json::to_string(&msg) {
                    Ok(j) => j,
                    Err(_) => continue,
                };

                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Text(text))) => {
                        if let Ok(cmd) = serde_json::from_str::<TerminalCommand>(text.as_str()) {
                            match cmd {
                                TerminalCommand::Input { data } => {
                                    if let Ok(bytes) = BASE64.decode(&data) {
                                        let _ = pty_service.write(session_id_for_input, &bytes).await;
                                    }
                                }
                                TerminalCommand::Resize { cols, rows } => {
                                    let _ = pty_service.resize(session_id_for_input, cols, rows).await;
                                }
                            }
                        }
                    }
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(error) => {
                        tracing::warn!("terminal WS receive error: {}", error);
                        break;
                    }
                }
            }
        }
    }

    let _ = deployment.pty().close_session(session_id).await;
}

async fn send_error(socket: &mut MaybeSignedWebSocket, message: &str) -> anyhow::Result<()> {
    let msg = TerminalMessage::Error {
        message: message.to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap_or_default();
    socket.send(Message::Text(json.into())).await?;
    socket.close().await?;
    Ok(())
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/terminal/ws", get(terminal_ws))
}

#[cfg(test)]
mod tests {
    use db::models::workspace::WorkspacePlacementState;

    use super::*;

    #[test]
    fn terminal_uses_persisted_worker_affinity() {
        let worker_node_id = Uuid::new_v4();
        let placement = WorkspacePlacement {
            workspace_id: Uuid::new_v4(),
            worker_node_id: Some(worker_node_id),
            placement_state: WorkspacePlacementState::Ready,
            placed_at: None,
            placement_reason: None,
            requested_worker_node_id: None,
            placement_constraints: None,
        };
        assert_eq!(terminal_worker_id(Some(placement)), Some(worker_node_id));
        assert_eq!(terminal_worker_id(None), None);
    }
}
