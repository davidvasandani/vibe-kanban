use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use executors::mcp_refresh::{
    McpRefreshErrorCategory, McpRefreshResult, McpRefreshStatus, McpServerRefreshSnapshot,
    safe_executor_error,
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct McpRefreshCoordinator {
    states: Arc<RwLock<HashMap<Uuid, McpRefreshResult>>>,
}

impl McpRefreshCoordinator {
    pub async fn request(&self, session_id: Uuid, supported: bool) -> McpRefreshResult {
        let mut states = self.states.write().await;
        if let Some(current) = states.get(&session_id)
            && matches!(current.status, McpRefreshStatus::PendingNextTurn)
        {
            let mut busy = current.clone();
            busy.status = McpRefreshStatus::Busy;
            busy.retryable = true;
            busy.error = Some(safe_executor_error(
                McpRefreshErrorCategory::RefreshInProgress,
            ));
            return busy;
        }

        let previous = states.get(&session_id);
        let generation = previous.map_or(1, |state| state.generation + 1);
        let now = Utc::now();
        let result = if supported {
            McpRefreshResult {
                status: McpRefreshStatus::PendingNextTurn,
                retryable: false,
                generation,
                requested_at: now,
                last_successful_refresh_at: previous
                    .and_then(|state| state.last_successful_refresh_at),
                servers: previous.map_or_else(Vec::new, |state| state.servers.clone()),
                error: None,
            }
        } else {
            McpRefreshResult {
                status: McpRefreshStatus::Unsupported,
                retryable: false,
                generation,
                requested_at: now,
                last_successful_refresh_at: previous
                    .and_then(|state| state.last_successful_refresh_at),
                servers: previous.map_or_else(Vec::new, |state| state.servers.clone()),
                error: Some(safe_executor_error(McpRefreshErrorCategory::Unsupported)),
            }
        };
        states.insert(session_id, result.clone());
        result
    }

    pub async fn fail(
        &self,
        session_id: Uuid,
        category: McpRefreshErrorCategory,
    ) -> Option<McpRefreshResult> {
        let mut states = self.states.write().await;
        let state = states.get_mut(&session_id)?;
        state.status = McpRefreshStatus::Failed;
        state.retryable = true;
        state.error = Some(safe_executor_error(category));
        Some(state.clone())
    }

    pub async fn busy(&self, session_id: Uuid) -> Option<McpRefreshResult> {
        let state = self.states.read().await.get(&session_id)?.clone();
        let mut busy = state;
        busy.status = McpRefreshStatus::Busy;
        busy.retryable = true;
        busy.error = Some(safe_executor_error(
            McpRefreshErrorCategory::RefreshInProgress,
        ));
        Some(busy)
    }

    pub async fn unsupported(&self, session_id: Uuid) -> Option<McpRefreshResult> {
        let mut states = self.states.write().await;
        let state = states.get_mut(&session_id)?;
        state.status = McpRefreshStatus::Unsupported;
        state.retryable = false;
        state.error = Some(safe_executor_error(McpRefreshErrorCategory::Unsupported));
        Some(state.clone())
    }

    pub async fn confirm(
        &self,
        session_id: Uuid,
        mut servers: Vec<McpServerRefreshSnapshot>,
    ) -> Option<McpRefreshResult> {
        let mut states = self.states.write().await;
        let state = states.get_mut(&session_id)?;
        if !matches!(state.status, McpRefreshStatus::PendingNextTurn) {
            return Some(state.clone());
        }
        servers.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        let partial = servers.iter().any(|server| {
            matches!(
                server.status,
                executors::mcp_refresh::McpServerRefreshStatus::FailedRetained
                    | executors::mcp_refresh::McpServerRefreshStatus::FailedUnavailable
            )
        });
        state.status = if partial {
            McpRefreshStatus::PartiallyRefreshed
        } else {
            McpRefreshStatus::Refreshed
        };
        state.retryable = false;
        state.servers = servers;
        state.error = None;
        if !partial {
            state.last_successful_refresh_at = Some(Utc::now());
        }
        Some(state.clone())
    }

    pub async fn status(&self, session_id: Uuid) -> Option<McpRefreshResult> {
        self.states.read().await.get(&session_id).cloned()
    }

    pub async fn remove(&self, session_id: Uuid) {
        self.states.write().await.remove(&session_id);
    }
}

#[cfg(test)]
mod tests {
    use executors::mcp_refresh::{McpServerRefreshSnapshot, McpServerRefreshStatus};

    use super::*;

    #[tokio::test]
    async fn concurrent_request_is_retryable_busy() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let first = coordinator.request(session, true).await;
        let second = coordinator.request(session, true).await;
        assert_eq!(first.status, McpRefreshStatus::PendingNextTurn);
        assert_eq!(second.status, McpRefreshStatus::Busy);
        assert!(second.retryable);
    }

    #[tokio::test]
    async fn unsupported_does_not_claim_pending_or_success() {
        let result = McpRefreshCoordinator::default()
            .request(Uuid::new_v4(), false)
            .await;
        assert_eq!(result.status, McpRefreshStatus::Unsupported);
        assert!(!result.retryable);
    }

    #[tokio::test]
    async fn failed_server_is_not_claimed_as_retained_without_executor_support() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        coordinator.request(session, true).await;
        coordinator
            .confirm(
                session,
                vec![McpServerRefreshSnapshot {
                    server_id: "slack".to_string(),
                    status: McpServerRefreshStatus::Ready,
                    tool_count: Some(7),
                    tool_names: Some(vec!["attachment_get_data".to_string()]),
                    tool_schema_fingerprint: Some("generation-a".to_string()),
                    resource_count: Some(2),
                    prompt_count: None,
                    restart_occurred: None,
                    error: None,
                }],
            )
            .await;
        coordinator.request(session, true).await;
        let result = coordinator
            .confirm(
                session,
                vec![McpServerRefreshSnapshot {
                    server_id: "slack".to_string(),
                    status: McpServerRefreshStatus::FailedUnavailable,
                    tool_count: Some(0),
                    tool_names: None,
                    tool_schema_fingerprint: None,
                    resource_count: Some(0),
                    prompt_count: None,
                    restart_occurred: None,
                    error: Some(safe_executor_error(
                        McpRefreshErrorCategory::AuthenticationFailed,
                    )),
                }],
            )
            .await
            .unwrap();
        assert_eq!(result.status, McpRefreshStatus::PartiallyRefreshed);
        assert_eq!(
            result.servers[0].status,
            McpServerRefreshStatus::FailedUnavailable
        );
        assert_eq!(result.servers[0].tool_count, Some(0));
    }

    #[tokio::test]
    async fn each_confirmed_generation_replaces_exact_tool_evidence() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();

        for (names, fingerprint) in [
            (
                vec!["sn_access_cycle_report".to_string()],
                "generation-original",
            ),
            (
                vec![
                    "entra_user_lookup".to_string(),
                    "sn_access_cycle_report".to_string(),
                ],
                "generation-added",
            ),
            (vec!["entra_user_lookup".to_string()], "generation-removed"),
            (
                vec!["entra_user_lookup".to_string()],
                "generation-schema-changed",
            ),
        ] {
            coordinator.request(session, true).await;
            let result = coordinator
                .confirm(
                    session,
                    vec![McpServerRefreshSnapshot {
                        server_id: "personal_servicenow".to_string(),
                        status: McpServerRefreshStatus::Ready,
                        tool_count: Some(names.len() as u32),
                        tool_names: Some(names.clone()),
                        tool_schema_fingerprint: Some(fingerprint.to_string()),
                        resource_count: Some(0),
                        prompt_count: None,
                        restart_occurred: None,
                        error: None,
                    }],
                )
                .await
                .unwrap();

            assert_eq!(result.servers.len(), 1);
            assert_eq!(result.servers[0].tool_names.as_ref(), Some(&names));
            assert_eq!(
                result.servers[0].tool_schema_fingerprint.as_deref(),
                Some(fingerprint)
            );
        }
    }
}
