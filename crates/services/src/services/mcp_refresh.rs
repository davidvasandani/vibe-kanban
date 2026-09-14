use std::{collections::HashMap, sync::Arc};

use chrono::Utc;
use executors::mcp_refresh::{
    McpDiscoveryObservation, McpRefreshErrorCategory, McpRefreshResult, McpRefreshStatus,
    McpServerRefreshSnapshot, safe_executor_error,
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct McpRefreshCoordinator {
    states: Arc<RwLock<HashMap<Uuid, McpRefreshResult>>>,
    generations: Arc<RwLock<HashMap<Uuid, u64>>>,
}

impl McpRefreshCoordinator {
    /// Publish the inventory observed by an ordinary executor startup. This
    /// keeps status tied to the active process even when no refresh was queued.
    /// A pending refresh owns its generation and cannot be displaced here.
    pub async fn observe_inventory(
        &self,
        session_id: Uuid,
        execution_started_at: chrono::DateTime<Utc>,
        mut configured_server_ids: Vec<String>,
        mut servers: Vec<McpServerRefreshSnapshot>,
    ) -> McpRefreshResult {
        let mut states = self.states.write().await;
        if let Some(current) = states.get(&session_id)
            && (current.status == McpRefreshStatus::PendingNextTurn
                || current.requested_at > execution_started_at)
        {
            return current.clone();
        }
        configured_server_ids.sort();
        configured_server_ids.dedup();
        let now = Utc::now();
        for configured_id in &configured_server_ids {
            if !servers
                .iter()
                .any(|server| &server.server_id == configured_id)
            {
                servers.push(McpServerRefreshSnapshot {
                    server_id: configured_id.clone(),
                    status: executors::mcp_refresh::McpServerRefreshStatus::NotRegistered,
                    tool_count: Some(0),
                    tool_names: Some(Vec::new()),
                    tool_schema_fingerprint: None,
                    resource_count: None,
                    prompt_count: None,
                    restart_occurred: Some(false),
                    discovery_attempts: 1,
                    observed_errors: Vec::new(),
                    first_observed_at: Some(now),
                    last_observed_at: Some(now),
                    terminal_at: Some(now),
                    error: Some(safe_executor_error(
                        McpRefreshErrorCategory::CapabilityListFailed,
                    )),
                });
            }
        }
        servers.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        let partial = servers.iter().any(|server| {
            matches!(
                server.status,
                executors::mcp_refresh::McpServerRefreshStatus::FailedRetained
                    | executors::mcp_refresh::McpServerRefreshStatus::FailedUnavailable
                    | executors::mcp_refresh::McpServerRefreshStatus::NotRegistered
            )
        });
        let generation = {
            let mut generations = self.generations.write().await;
            let generation = generations.entry(session_id).or_default();
            *generation = generation.saturating_add(1);
            *generation
        };
        let result = McpRefreshResult {
            status: if partial {
                McpRefreshStatus::PartiallyRefreshed
            } else {
                McpRefreshStatus::Refreshed
            },
            retryable: false,
            generation,
            requested_at: execution_started_at,
            last_successful_refresh_at: (!partial).then_some(now),
            configured_server_ids,
            servers,
            error: None,
        };
        states.insert(session_id, result.clone());
        result
    }

    pub async fn request(
        &self,
        session_id: Uuid,
        supported: bool,
        configured_server_ids: Vec<String>,
    ) -> McpRefreshResult {
        self.request_inner(session_id, supported, configured_server_ids, false)
            .await
    }

    /// A fresh process supersedes a wedged live-refresh generation. Unlike a
    /// second refresh click, restart must not return `busy` forever merely
    /// because the generation it is intended to recover never completed.
    pub async fn request_restart(
        &self,
        session_id: Uuid,
        configured_server_ids: Vec<String>,
    ) -> McpRefreshResult {
        self.request_inner(session_id, true, configured_server_ids, true)
            .await
    }

    async fn request_inner(
        &self,
        session_id: Uuid,
        supported: bool,
        mut configured_server_ids: Vec<String>,
        supersede_pending: bool,
    ) -> McpRefreshResult {
        configured_server_ids.sort();
        configured_server_ids.dedup();
        let mut states = self.states.write().await;
        if let Some(current) = states.get(&session_id)
            && matches!(current.status, McpRefreshStatus::PendingNextTurn)
            && !supersede_pending
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
        let generation = {
            let mut generations = self.generations.write().await;
            let generation = generations.entry(session_id).or_default();
            *generation = generation.saturating_add(1);
            *generation
        };
        let now = Utc::now();
        let connecting_servers = configured_server_ids
            .iter()
            .map(|server_id| McpServerRefreshSnapshot {
                server_id: server_id.clone(),
                status: executors::mcp_refresh::McpServerRefreshStatus::Connecting,
                tool_count: None,
                tool_names: None,
                tool_schema_fingerprint: None,
                resource_count: None,
                prompt_count: None,
                restart_occurred: None,
                discovery_attempts: 0,
                observed_errors: Vec::new(),
                first_observed_at: Some(now),
                last_observed_at: Some(now),
                terminal_at: None,
                error: None,
            })
            .collect();
        let result = if supported {
            McpRefreshResult {
                status: McpRefreshStatus::PendingNextTurn,
                retryable: false,
                generation,
                requested_at: now,
                last_successful_refresh_at: previous
                    .and_then(|state| state.last_successful_refresh_at),
                configured_server_ids,
                servers: connecting_servers,
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
                configured_server_ids,
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
        expected_generation: u64,
        category: McpRefreshErrorCategory,
    ) -> Option<McpRefreshResult> {
        let mut states = self.states.write().await;
        let state = states.get_mut(&session_id)?;
        if state.generation != expected_generation {
            return None;
        }
        let now = Utc::now();
        for server in &mut state.servers {
            if server.status == executors::mcp_refresh::McpServerRefreshStatus::Connecting {
                server.status = executors::mcp_refresh::McpServerRefreshStatus::FailedUnavailable;
                server.discovery_attempts = server.discovery_attempts.saturating_add(1);
                server.last_observed_at = Some(now);
                server.terminal_at = Some(now);
                server.observed_errors.push(McpDiscoveryObservation {
                    code: category.clone(),
                    observed_at: now,
                });
                server.error = Some(safe_executor_error(category.clone()));
            }
        }
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
        expected_generation: u64,
        mut servers: Vec<McpServerRefreshSnapshot>,
    ) -> Option<McpRefreshResult> {
        let mut states = self.states.write().await;
        let state = states.get_mut(&session_id)?;
        if state.generation != expected_generation {
            return None;
        }
        if !matches!(state.status, McpRefreshStatus::PendingNextTurn) {
            return Some(state.clone());
        }
        for configured_id in &state.configured_server_ids {
            if !servers
                .iter()
                .any(|server| &server.server_id == configured_id)
            {
                servers.push(McpServerRefreshSnapshot {
                    server_id: configured_id.clone(),
                    status: executors::mcp_refresh::McpServerRefreshStatus::NotRegistered,
                    tool_count: Some(0),
                    tool_names: Some(Vec::new()),
                    tool_schema_fingerprint: None,
                    resource_count: None,
                    prompt_count: None,
                    restart_occurred: Some(true),
                    discovery_attempts: 1,
                    observed_errors: Vec::new(),
                    first_observed_at: Some(Utc::now()),
                    last_observed_at: Some(Utc::now()),
                    terminal_at: Some(Utc::now()),
                    error: Some(safe_executor_error(
                        McpRefreshErrorCategory::CapabilityListFailed,
                    )),
                });
            }
        }
        servers.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        let partial = servers.iter().any(|server| {
            matches!(
                server.status,
                executors::mcp_refresh::McpServerRefreshStatus::FailedRetained
                    | executors::mcp_refresh::McpServerRefreshStatus::FailedUnavailable
                    | executors::mcp_refresh::McpServerRefreshStatus::NotRegistered
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
        let first = coordinator
            .request(session, true, vec!["slack".into()])
            .await;
        let second = coordinator
            .request(session, true, vec!["slack".into()])
            .await;
        assert_eq!(first.status, McpRefreshStatus::PendingNextTurn);
        assert_eq!(second.status, McpRefreshStatus::Busy);
        assert!(second.retryable);
    }

    #[tokio::test]
    async fn unsupported_does_not_claim_pending_or_success() {
        let result = McpRefreshCoordinator::default()
            .request(Uuid::new_v4(), false, Vec::new())
            .await;
        assert_eq!(result.status, McpRefreshStatus::Unsupported);
        assert!(!result.retryable);
    }

    #[tokio::test]
    async fn request_exposes_only_sorted_configured_server_ids() {
        let result = McpRefreshCoordinator::default()
            .request(
                Uuid::new_v4(),
                true,
                vec!["slack".into(), "logmein".into(), "slack".into()],
            )
            .await;
        assert_eq!(result.configured_server_ids, ["logmein", "slack"]);
        assert_eq!(result.servers.len(), 2);
        assert!(
            result
                .servers
                .iter()
                .all(|server| server.status == McpServerRefreshStatus::Connecting)
        );
    }

    #[tokio::test]
    async fn configured_server_missing_from_fresh_registry_is_terminal() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let pending = coordinator
            .request_restart(session, vec!["slack".into()])
            .await;

        let result = coordinator
            .confirm(session, pending.generation, Vec::new())
            .await
            .unwrap();

        assert_eq!(result.status, McpRefreshStatus::PartiallyRefreshed);
        assert_eq!(result.servers[0].server_id, "slack");
        assert_eq!(
            result.servers[0].status,
            McpServerRefreshStatus::NotRegistered
        );
        assert_eq!(result.servers[0].tool_count, Some(0));
        assert!(result.servers[0].terminal_at.is_some());
    }

    #[tokio::test]
    async fn discovery_timeout_terminalizes_every_connecting_server() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let pending = coordinator
            .request_restart(session, vec!["brink".into(), "slack".into()])
            .await;

        let result = coordinator
            .fail(
                session,
                pending.generation,
                McpRefreshErrorCategory::Timeout,
            )
            .await
            .unwrap();

        assert_eq!(result.status, McpRefreshStatus::Failed);
        assert!(result.servers.iter().all(|server| {
            server.status == McpServerRefreshStatus::FailedUnavailable
                && server.discovery_attempts == 1
                && server.observed_errors[0].code == McpRefreshErrorCategory::Timeout
                && server.terminal_at.is_some()
        }));
    }

    #[tokio::test]
    async fn stale_inventory_cannot_complete_a_superseding_restart_generation() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let stale = coordinator
            .request(session, true, vec!["slack".into()])
            .await;
        let current = coordinator
            .request_restart(session, vec!["slack".into()])
            .await;

        assert!(
            coordinator
                .confirm(session, stale.generation, Vec::new())
                .await
                .is_none()
        );
        assert_eq!(
            coordinator.status(session).await.unwrap().generation,
            current.generation
        );
        assert_eq!(
            coordinator.status(session).await.unwrap().status,
            McpRefreshStatus::PendingNextTurn
        );
    }

    #[tokio::test]
    async fn clearing_status_does_not_reuse_a_generation() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let first = coordinator
            .request_restart(session, vec!["slack".into()])
            .await;
        coordinator.remove(session).await;
        let second = coordinator
            .request_restart(session, vec!["slack".into()])
            .await;

        assert!(second.generation > first.generation);
        assert!(
            coordinator
                .confirm(session, first.generation, Vec::new())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn ordinary_inventory_replaces_a_stale_terminal_snapshot() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let pending = coordinator
            .request_restart(session, vec!["slack".into()])
            .await;
        coordinator
            .fail(
                session,
                pending.generation,
                McpRefreshErrorCategory::Timeout,
            )
            .await;

        let observed = coordinator
            .observe_inventory(
                session,
                Utc::now(),
                vec!["brink".into(), "slack".into()],
                vec![McpServerRefreshSnapshot {
                    server_id: "slack".into(),
                    status: McpServerRefreshStatus::Ready,
                    tool_count: Some(12),
                    tool_names: Some(Vec::new()),
                    tool_schema_fingerprint: None,
                    resource_count: Some(0),
                    prompt_count: Some(0),
                    restart_occurred: None,
                    discovery_attempts: 1,
                    observed_errors: Vec::new(),
                    first_observed_at: None,
                    last_observed_at: None,
                    terminal_at: None,
                    error: None,
                }],
            )
            .await;

        assert!(observed.generation > pending.generation);
        assert_eq!(observed.status, McpRefreshStatus::PartiallyRefreshed);
        assert_eq!(observed.servers[0].server_id, "brink");
        assert_eq!(
            observed.servers[0].status,
            McpServerRefreshStatus::NotRegistered
        );
        assert_eq!(observed.servers[1].tool_count, Some(12));
    }

    #[tokio::test]
    async fn failed_server_is_not_claimed_as_retained_without_executor_support() {
        let coordinator = McpRefreshCoordinator::default();
        let session = Uuid::new_v4();
        let first = coordinator
            .request(session, true, vec!["slack".into()])
            .await;
        coordinator
            .confirm(
                session,
                first.generation,
                vec![McpServerRefreshSnapshot {
                    server_id: "slack".to_string(),
                    status: McpServerRefreshStatus::Ready,
                    tool_count: Some(7),
                    tool_names: Some(vec!["attachment_get_data".to_string()]),
                    tool_schema_fingerprint: Some("generation-a".to_string()),
                    resource_count: Some(2),
                    prompt_count: None,
                    restart_occurred: None,
                    discovery_attempts: 1,
                    observed_errors: Vec::new(),
                    first_observed_at: None,
                    last_observed_at: None,
                    terminal_at: None,
                    error: None,
                }],
            )
            .await;
        let second = coordinator
            .request(session, true, vec!["slack".into()])
            .await;
        let result = coordinator
            .confirm(
                session,
                second.generation,
                vec![McpServerRefreshSnapshot {
                    server_id: "slack".to_string(),
                    status: McpServerRefreshStatus::FailedUnavailable,
                    tool_count: Some(0),
                    tool_names: None,
                    tool_schema_fingerprint: None,
                    resource_count: Some(0),
                    prompt_count: None,
                    restart_occurred: None,
                    discovery_attempts: 1,
                    observed_errors: Vec::new(),
                    first_observed_at: None,
                    last_observed_at: None,
                    terminal_at: None,
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
            let pending = coordinator
                .request(session, true, vec!["personal_servicenow".to_string()])
                .await;
            let result = coordinator
                .confirm(
                    session,
                    pending.generation,
                    vec![McpServerRefreshSnapshot {
                        server_id: "personal_servicenow".to_string(),
                        status: McpServerRefreshStatus::Ready,
                        tool_count: Some(names.len() as u32),
                        tool_names: Some(names.clone()),
                        tool_schema_fingerprint: Some(fingerprint.to_string()),
                        resource_count: Some(0),
                        prompt_count: None,
                        restart_occurred: None,
                        discovery_attempts: 1,
                        observed_errors: Vec::new(),
                        first_observed_at: None,
                        last_observed_at: None,
                        terminal_at: None,
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
