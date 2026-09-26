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
    // Discovery may use the whole request budget (a caller that does not
    // wait still needs to learn which runs started, or it has nothing to
    // poll); waiting afterwards honours `wait_secs`. Both end by the same
    // ceiling, so an agent's call stays inside its tool deadline.
    let started_at = tokio::time::Instant::now();
    let discovery_deadline = started_at + clamp_wait(MAX_WAIT_SECS);
    let deadline = started_at + clamp_wait(request.wait_secs);
    let reports = match request.target.as_deref() {
        Some(target) => match target
            .parse::<ReauthTargetId>()
            .and_then(|id| reauth::start_listed(id, trigger))
        {
            Ok(report) => vec![report],
            Err(e) => return ResponseJson(ApiResponse::error(&e.to_string())),
        },
        None => {
            // Discovery probes every target; run it detached so a slow probe
            // cannot hold the request past its deadline. It keeps going (and
            // starts whatever it finds) if we stop watching.
            let discovery = tokio::spawn(reauth::start_expired(trigger));
            match tokio::time::timeout_at(discovery_deadline, discovery).await {
                Ok(Ok(reports)) => reports,
                Ok(Err(e)) => {
                    return ResponseJson(ApiResponse::error(&format!(
                        "re-auth discovery failed: {e}"
                    )));
                }
                Err(_) => {
                    return ResponseJson(ApiResponse::error(
                        "still checking which credentials are expired; any repairs start on \
                         their own. Check list_reauth_targets in a minute.",
                    ));
                }
            }
        }
    };
    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
    let reports = reauth::wait(reports, remaining).await;
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

    #[tokio::test]
    async fn an_unlisted_target_is_rejected_before_anything_starts() {
        // A well-formed backend that this host does not list (no
        // VK_SGSC_BACKENDS in tests) must not start an invisible run.
        let ResponseJson(resp) = run(axum::Json(RunRequest {
            target: Some("sgsc:unlisted".into()),
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
                .contains("not a re-auth target on this host"),
            "{json}"
        );
    }
}
