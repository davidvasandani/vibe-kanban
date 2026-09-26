//! Unattended re-authentication endpoints (see services::reauth).
//!
//! Callers name a server-defined target and nothing else: no URL, command or
//! credential reference ever comes from the request. Runs are owned by the
//! service, so `wait_secs` only bounds how long this request watches one.

use std::time::Duration;

use axum::{
    Router,
    response::Json as ResponseJson,
    routing::{get, post},
};
use serde::Deserialize;
use services::services::reauth::{
    self, ReauthOverview, ReauthRunReport, ReauthTargetId, ReauthTrigger,
};
use utils::response::ApiResponse;

use crate::DeploymentImpl;

/// Longest a request may watch a run: under a 60 s agent tool deadline.
const MAX_WAIT_SECS: u32 = 55;

#[derive(Debug, Deserialize)]
struct RunRequest {
    /// Omit to repair every swept target that is currently unauthenticated.
    target: Option<String>,
    #[serde(default)]
    wait_secs: u32,
    /// Defaults to `agent`. Only `manual` (the Settings UI) may retry a
    /// target that was refused earlier.
    #[serde(default = "default_trigger")]
    trigger: ReauthTrigger,
}

fn default_trigger() -> ReauthTrigger {
    ReauthTrigger::Agent
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/reauth/targets", get(list_targets))
        .route("/reauth/runs", get(list_runs))
        .route("/reauth/run", post(run))
}

async fn list_targets() -> ResponseJson<ApiResponse<ReauthOverview>> {
    ResponseJson(ApiResponse::success(reauth::list_targets().await))
}

/// Last runs only, without probing: what a UI polls while a run is active.
async fn list_runs() -> ResponseJson<ApiResponse<Vec<ReauthRunReport>>> {
    ResponseJson(ApiResponse::success(reauth::recent_runs()))
}

fn clamp_wait(secs: u32) -> Duration {
    Duration::from_secs(u64::from(secs.min(MAX_WAIT_SECS)))
}

async fn run(
    axum::Json(request): axum::Json<RunRequest>,
) -> ResponseJson<ApiResponse<Vec<ReauthRunReport>>> {
    // The sweep is the service's own loop; an HTTP caller never speaks for it.
    let trigger = match request.trigger {
        ReauthTrigger::Sweep => ReauthTrigger::Agent,
        other => other,
    };
    let reports = match request.target.as_deref() {
        Some(target) => match target.parse::<ReauthTargetId>() {
            Ok(id) => vec![reauth::start(id, trigger)],
            Err(e) => return ResponseJson(ApiResponse::error(&e.to_string())),
        },
        None => reauth::start_expired(trigger).await,
    };
    let reports = reauth::wait(reports, clamp_wait(request.wait_secs)).await;
    ResponseJson(ApiResponse::success(reports))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_is_clamped_under_agent_tool_deadlines() {
        assert_eq!(clamp_wait(0), Duration::ZERO);
        assert_eq!(clamp_wait(50), Duration::from_secs(50));
        assert_eq!(clamp_wait(3600), Duration::from_secs(55));
    }

    #[test]
    fn request_defaults_to_the_agent_trigger() {
        let r: RunRequest = serde_json::from_str(r#"{"target":"sgsc:dp"}"#).unwrap();
        assert_eq!(r.trigger, ReauthTrigger::Agent);
        assert_eq!(r.wait_secs, 0);
        let r: RunRequest = serde_json::from_str(r#"{"trigger":"manual"}"#).unwrap();
        assert_eq!(r.trigger, ReauthTrigger::Manual);
        assert!(r.target.is_none());
    }

    #[tokio::test]
    async fn an_invalid_target_is_rejected_before_anything_starts() {
        let ResponseJson(resp) = run(axum::Json(RunRequest {
            target: Some("shell:rm -rf /".into()),
            wait_secs: 0,
            trigger: ReauthTrigger::Agent,
        }))
        .await;
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["success"], false);
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("unknown target kind")
        );
    }
}
