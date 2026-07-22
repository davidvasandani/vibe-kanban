//! AWS SSO profile management endpoints (see services::aws_sso).
//!
//! Profiles live in the user's own `~/.aws/config`; VK edits it as a guest
//! and never stores credentials. Sign-in streams the vendor's own
//! `aws sso login --profile <name>` over a signed PTY WebSocket, mirroring
//! the CLI-tools login flow (same wire framing), and only an independent
//! `sts get-caller-identity` probe turns a clean exit into a success.

use axum::{
    Router,
    extract::{Path, Query, State, ws::Message},
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post, put},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};
use services::services::aws_sso::{
    self, AwsAuthStatus, AwsProfileImportRequest, AwsProfileImportResult, AwsSsoAccount,
    AwsSsoProfile, AwsSsoProfileStatus, AwsSsoSession,
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
        profile: Box<AwsSsoProfileStatus>,
    },
    Error {
        code: &'static str,
        message: String,
    },
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/aws/profiles", get(list_aws_profiles))
        .route(
            "/aws/profiles/{name}",
            put(save_aws_profile).delete(delete_aws_profile),
        )
        .route("/aws/profiles/{name}/login/ws", get(login_aws_profile))
        .route("/aws/profiles/import", post(import_aws_profiles))
        .route("/aws/sso/sessions", get(list_aws_sessions))
        .route("/aws/sso/sessions/{name}", put(prepare_aws_session))
        .route(
            "/aws/sso/sessions/{name}/catalog",
            get(discover_aws_catalog),
        )
        .route("/aws/sso/sessions/{name}/login/ws", get(login_aws_session))
}

async fn list_aws_sessions() -> ResponseJson<ApiResponse<Vec<AwsSsoSession>>> {
    match aws_sso::list_sessions() {
        Ok(value) => ResponseJson(ApiResponse::success(value)),
        Err(e) => ResponseJson(ApiResponse::error(&e.to_string())),
    }
}

async fn prepare_aws_session(
    Path(name): Path<String>,
    axum::Json(session): axum::Json<AwsSsoSession>,
) -> ResponseJson<ApiResponse<AwsSsoSession>> {
    if name != session.name {
        return ResponseJson(ApiResponse::error(
            "session name in path does not match body",
        ));
    }
    match aws_sso::prepare_session(&session).await {
        Ok(value) => ResponseJson(ApiResponse::success(value)),
        Err(e) => ResponseJson(ApiResponse::error(&e.to_string())),
    }
}

async fn discover_aws_catalog(
    Path(name): Path<String>,
) -> ResponseJson<ApiResponse<Vec<AwsSsoAccount>>> {
    match aws_sso::discover_catalog(&name).await {
        Ok(value) => ResponseJson(ApiResponse::success(value)),
        Err(e) => ResponseJson(ApiResponse::error(&e.to_string())),
    }
}

async fn import_aws_profiles(
    axum::Json(request): axum::Json<AwsProfileImportRequest>,
) -> ResponseJson<ApiResponse<AwsProfileImportResult>> {
    match aws_sso::import_profiles(&request).await {
        Ok(value) => ResponseJson(ApiResponse::success(value)),
        Err(e) => ResponseJson(ApiResponse::error(&e.to_string())),
    }
}

async fn list_aws_profiles() -> ResponseJson<ApiResponse<Vec<AwsSsoProfileStatus>>> {
    match aws_sso::list_profile_statuses().await {
        Ok(profiles) => ResponseJson(ApiResponse::success(profiles)),
        Err(e) => {
            tracing::error!("AWS profile listing failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}

/// The path names the profile being written; a body that names a different
/// profile is a client bug, not a rename operation.
fn ensure_name_matches(path_name: &str, profile: &AwsSsoProfile) -> Result<(), String> {
    if path_name != profile.name {
        return Err(format!(
            "profile name in path (`{path_name}`) does not match body (`{}`)",
            profile.name
        ));
    }
    Ok(())
}

async fn save_aws_profile(
    Path(name): Path<String>,
    axum::Json(profile): axum::Json<AwsSsoProfile>,
) -> ResponseJson<ApiResponse<AwsSsoProfile>> {
    if let Err(message) = ensure_name_matches(&name, &profile) {
        return ResponseJson(ApiResponse::error(&message));
    }
    match aws_sso::upsert_profile(&profile).await {
        Ok(saved) => ResponseJson(ApiResponse::success(saved)),
        Err(e) => {
            tracing::error!("AWS profile save failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}

async fn delete_aws_profile(Path(name): Path<String>) -> ResponseJson<ApiResponse<()>> {
    match aws_sso::delete_profile(&name).await {
        Ok(()) => ResponseJson(ApiResponse::success(())),
        Err(e) => {
            tracing::error!("AWS profile delete failed: {e}");
            ResponseJson(ApiResponse::error(&e.to_string()))
        }
    }
}

async fn login_aws_profile(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(name): Path<String>,
    Query(query): Query<LoginQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let command = aws_sso::login_command_for_profile(&name)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ws.on_upgrade(move |socket| handle_login(socket, deployment, Some(name), command, query)))
}

async fn login_aws_session(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(name): Path<String>,
    Query(query): Query<LoginQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let command = aws_sso::login_command_for_session(&name)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(ws.on_upgrade(move |socket| handle_login(socket, deployment, None, command, query)))
}

async fn handle_login(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    profile_name: Option<String>,
    command: aws_sso::AwsLoginCommand,
    query: LoginQuery,
) {
    let Some(_guard) = aws_sso::try_begin_profile_login(&command.lock_key) else {
        send_message(
            &mut socket,
            &LoginMessage::Error {
                code: "session_conflict",
                message: "Sign-in is already active for this profile".to_string(),
            },
        )
        .await;
        return;
    };
    // $HOME so tokens land in the AWS CLI's own ~/.aws/sso/cache.
    let working_dir = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let (session_id, mut output_rx, mut exit_rx) = match deployment
        .pty()
        .create_command_session(
            command.executable,
            command.args,
            working_dir,
            command.env,
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

    // Normal completion: the waiter already reaped the child, so remove the
    // session without signalling the cloned PID. Everything else kills it.
    if child_exited {
        let _ = pty.finish_session(session_id).await;
    } else {
        let _ = pty.close_session(session_id).await;
    }
    // Command exit and verified authentication are distinct facts: a zero
    // exit only becomes success once the independent probe confirms it.
    let status = match &profile_name {
        Some(name) => aws_sso::profile_status(name).await.ok(),
        None => None,
    };
    if profile_name.is_some()
        && outcome == "succeeded"
        && !matches!(
            status.as_ref().map(|s| &s.auth),
            Some(AwsAuthStatus::Authenticated { .. })
        )
    {
        outcome = "verification_failed";
    }
    send_message(&mut socket, &LoginMessage::Exit { outcome, exit_code }).await;
    if let Some(status) = status {
        send_message(
            &mut socket,
            &LoginMessage::Status {
                profile: Box::new(status),
            },
        )
        .await;
    }
    let _ = socket.close().await;
}

async fn send_message(socket: &mut MaybeSignedWebSocket, message: &LoginMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        let _ = socket.send(Message::Text(json.into())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str) -> AwsSsoProfile {
        AwsSsoProfile {
            name: name.to_string(),
            sso_start_url: "https://org.awsapps.com/start".to_string(),
            sso_region: "us-east-1".to_string(),
            sso_account_id: "123456789012".to_string(),
            sso_role_name: "AdministratorAccess".to_string(),
            region: None,
            output: None,
        }
    }

    #[test]
    fn path_and_body_names_must_match() {
        assert!(ensure_name_matches("org.Admin", &profile("org.Admin")).is_ok());
        let err = ensure_name_matches("org.Admin", &profile("org.Other")).unwrap_err();
        assert!(err.contains("org.Admin") && err.contains("org.Other"));
    }

    #[tokio::test]
    async fn malformed_and_unknown_profile_names_are_rejected_before_upgrade() {
        // Charset rejection happens before any file or binary access.
        let err = aws_sso::login_command_for_profile("bad name; rm -rf /")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("profile name"));
        // Section-header injection cannot reach the config file.
        assert!(
            aws_sso::login_command_for_profile("x]\n[default")
                .await
                .is_err()
        );
    }

    #[test]
    fn default_profile_is_writable_by_neither_route_path() {
        // The service layer backs both PUT and DELETE; `default` is rejected
        // for writes while remaining a valid sign-in reference.
        assert!(aws_sso::validate_profile_name("default", false).is_err());
        assert!(aws_sso::validate_profile_name("default", true).is_ok());
    }

    #[test]
    fn concurrent_login_is_refused_without_spawning() {
        let guard = aws_sso::try_begin_profile_login("route-test.Admin").expect("first lock");
        assert!(aws_sso::try_begin_profile_login("route-test.Admin").is_none());
        drop(guard);
        assert!(aws_sso::try_begin_profile_login("route-test.Admin").is_some());
    }
}
