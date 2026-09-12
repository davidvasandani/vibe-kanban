//! App-managed CLI tool installer endpoints (see services::cli_tools).
//!
//! All outcomes — including download/verification failures — are returned
//! in-band via the ApiResponse envelope so the settings UI can surface them.

use std::future::Future;

use axum::{
    Router,
    extract::{Path, Query, State, ws::Message},
    response::{IntoResponse, Json as ResponseJson},
    routing::{delete, get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use services::services::{
    cli_tools::{self, CliToolId, CliToolStatus},
    entra_mint,
};
use utils::response::ApiResponse;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
};

const LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

#[derive(Debug, Deserialize)]
struct LoginQuery {
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
}

fn default_cols() -> u16 {
    80
}
fn default_rows() -> u16 {
    24
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LoginCommand {
    Input { data: String },
    Resize { cols: u16, rows: u16 },
    Cancel,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LoginMessage {
    Output {
        data: String,
    },
    Exit {
        outcome: &'static str,
        exit_code: Option<u32>,
    },
    Status {
        tool: Box<CliToolStatus>,
    },
    Error {
        code: &'static str,
        message: String,
    },
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/cli-tools", get(list_cli_tools))
        .route("/cli-tools/{id}/install", post(install_cli_tool))
        .route("/cli-tools/{id}/update", post(install_cli_tool))
        .route("/cli-tools/{id}/login/ws", get(login_cli_tool))
        .route("/cli-tools/{id}", delete(remove_cli_tool))
}

async fn login_cli_tool(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<CliToolId>,
    Query(query): Query<LoginQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let plan = cli_tools::login_plan(id)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ws.on_upgrade(move |socket| async move {
        match plan {
            cli_tools::CliToolLoginPlan::Command(command) => {
                handle_login(socket, deployment, id, command, query).await
            }
            cli_tools::CliToolLoginPlan::EntraMint { client_id, scope } => {
                handle_entra_login(socket, id, EntraFlow::Mint { client_id, scope }).await
            }
            cli_tools::CliToolLoginPlan::EntraNativeBrowser { args } => {
                handle_entra_login(socket, id, EntraFlow::NativeBrowser { args }).await
            }
        }
    }))
}

/// Entra sign-in has no PTY to attach to — it runs in-process and drives a
/// remote browser. Its progress is streamed over the same socket as terminal
/// output so the settings UI renders it exactly like a vendor login.
/// Which Entra sign-in shape a tool uses: we mint the token ourselves, or the
/// tool runs its own flow and we only supply the browser.
enum EntraFlow {
    Mint {
        client_id: &'static str,
        scope: &'static str,
    },
    NativeBrowser {
        args: Vec<String>,
    },
}

async fn handle_entra_login(mut socket: MaybeSignedWebSocket, id: CliToolId, flow: EntraFlow) {
    let Some(_guard) = cli_tools::try_begin_login(id) else {
        send_message(
            &mut socket,
            &LoginMessage::Error {
                code: "session_conflict",
                message: "Login is already active for this tool".to_string(),
            },
        )
        .await;
        return;
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let progress = entra_mint::Progress::new(tx);
    let mut task: std::pin::Pin<
        Box<dyn Future<Output = Result<(), cli_tools::CliToolError>> + Send>,
    > = match &flow {
        EntraFlow::Mint { client_id, scope } => {
            Box::pin(cli_tools::run_entra_login(id, client_id, scope, &progress))
        }
        EntraFlow::NativeBrowser { args } => Box::pin(cli_tools::run_entra_native_browser_login(
            id, args, &progress,
        )),
    };

    let timeout = tokio::time::sleep(LOGIN_TIMEOUT);
    tokio::pin!(timeout);
    let mut outcome = "cancelled";

    loop {
        tokio::select! {
            Some(line) = rx.recv() => {
                send_message(&mut socket, &LoginMessage::Output { data: BASE64.encode(line) }).await;
            }
            result = &mut task => {
                // Drain anything the mint emitted just before finishing.
                while let Ok(line) = rx.try_recv() {
                    send_message(&mut socket, &LoginMessage::Output { data: BASE64.encode(line) }).await;
                }
                outcome = match result {
                    Ok(()) => "succeeded",
                    Err(e) => {
                        send_message(
                            &mut socket,
                            &LoginMessage::Output { data: BASE64.encode(format!("\r\n{e}\r\n")) },
                        )
                        .await;
                        "command_failed"
                    }
                };
                break;
            }
            _ = &mut timeout => {
                outcome = "timed_out";
                break;
            }
            inbound = socket.recv() => match inbound {
                Ok(Some(Message::Text(text))) => {
                    if let Ok(LoginCommand::Cancel) = serde_json::from_str::<LoginCommand>(text.as_str()) {
                        break;
                    }
                }
                _ => break,
            },
        }
    }

    let tool = cli_tools::status(id).await;
    if outcome == "succeeded" && tool.auth_state != cli_tools::CliToolAuthState::Authenticated {
        outcome = "verification_failed";
    }
    send_message(
        &mut socket,
        &LoginMessage::Exit {
            outcome,
            exit_code: None,
        },
    )
    .await;
    send_message(
        &mut socket,
        &LoginMessage::Status {
            tool: Box::new(tool),
        },
    )
    .await;
    let _ = socket.close().await;
}

async fn handle_login(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    id: CliToolId,
    command: cli_tools::CliToolLoginCommand,
    query: LoginQuery,
) {
    let Some(_guard) = cli_tools::try_begin_login(id) else {
        send_message(
            &mut socket,
            &LoginMessage::Error {
                code: "session_conflict",
                message: "Login is already active for this tool".to_string(),
            },
        )
        .await;
        return;
    };
    let working_dir = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let (session_id, mut output_rx, mut exit_rx) = match deployment
        .pty()
        .create_command_session(
            command.executable,
            command.args,
            working_dir,
            std::collections::HashMap::new(),
            query.cols,
            query.rows,
            false,
        )
        .await
    {
        Ok(result) => result,
        Err(error) => {
            send_message(
                &mut socket,
                &LoginMessage::Error {
                    code: "spawn_failed",
                    message: error.to_string(),
                },
            )
            .await;
            return;
        }
    };
    let pty = deployment.pty().clone();
    let timeout = tokio::time::sleep(LOGIN_TIMEOUT);
    tokio::pin!(timeout);
    let mut outcome = "cancelled";
    let mut exit_code = None;
    let mut child_exited = false;

    loop {
        tokio::select! {
            Some(data) = output_rx.recv() => {
                send_message(&mut socket, &LoginMessage::Output { data: BASE64.encode(data) }).await;
            },
            exit = exit_rx.recv() => {
                exit_code = exit.map(|value| value.code);
                child_exited = exit_code.is_some();
                while let Ok(data) = output_rx.try_recv() {
                    send_message(&mut socket, &LoginMessage::Output { data: BASE64.encode(data) }).await;
                }
                outcome = if exit_code == Some(0) { "succeeded" } else { "command_failed" };
                break;
            }
            _ = &mut timeout => {
                outcome = "timed_out";
                break;
            }
            inbound = socket.recv() => match inbound {
                Ok(Some(Message::Text(text))) => {
                    if let Ok(command) = serde_json::from_str::<LoginCommand>(text.as_str()) {
                        match command {
                            LoginCommand::Input { data } => if let Ok(bytes) = BASE64.decode(data) { let _ = pty.write(session_id, &bytes).await; },
                            LoginCommand::Resize { cols, rows } => { let _ = pty.resize(session_id, cols, rows).await; }
                            LoginCommand::Cancel => break,
                        }
                    }
                }
                _ => break,
            }
        }
    }

    if child_exited {
        let _ = pty.finish_session(session_id).await;
    } else {
        let _ = pty.close_session(session_id).await;
    }
    let tool = cli_tools::status(id).await;
    if outcome == "succeeded" && tool.auth_state != cli_tools::CliToolAuthState::Authenticated {
        outcome = "verification_failed";
    }
    send_message(&mut socket, &LoginMessage::Exit { outcome, exit_code }).await;
    send_message(
        &mut socket,
        &LoginMessage::Status {
            tool: Box::new(tool),
        },
    )
    .await;
    let _ = socket.close().await;
}

async fn send_message(socket: &mut MaybeSignedWebSocket, message: &LoginMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        let _ = socket.send(Message::Text(json.into())).await;
    }
}

async fn list_cli_tools() -> ResponseJson<ApiResponse<Vec<CliToolStatus>>> {
    ResponseJson(ApiResponse::success(cli_tools::status_all().await))
}

async fn install_cli_tool(Path(id): Path<CliToolId>) -> ResponseJson<ApiResponse<CliToolStatus>> {
    match cli_tools::install(id).await {
        Ok(status) => ResponseJson(ApiResponse::success(status)),
        Err(e) => {
            tracing::error!("CLI tool install failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}

async fn remove_cli_tool(Path(id): Path<CliToolId>) -> ResponseJson<ApiResponse<CliToolStatus>> {
    match cli_tools::remove(id).await {
        Ok(status) => ResponseJson(ApiResponse::success(status)),
        Err(e) => {
            tracing::error!("CLI tool removal failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}
