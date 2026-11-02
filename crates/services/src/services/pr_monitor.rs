use std::{sync::Arc, time::Duration};

use api_types::{PullRequestStatus, UpdatePullRequestApiRequest, UpsertPullRequestRequest};
use chrono::Utc;
use db::{
    DBService,
    models::{
        merge::MergeStatus,
        pull_request::PullRequest,
        workspace::{Workspace, WorkspaceError},
    },
};
use git_host::{GitHostError, GitHostProvider, GitHostService};
use serde_json::json;
use sqlx::error::Error as SqlxError;
use thiserror::Error;
use tokio::{sync::Notify, time::interval};
use tracing::{debug, error, info, warn};

use crate::services::{
    analytics::AnalyticsContext,
    container::ContainerService,
    github_credentials,
    remote_client::{RemoteClient, RemoteClientError},
    remote_sync,
};

#[derive(Debug, Error)]
enum PrMonitorError {
    #[error(transparent)]
    GitHostError(#[from] GitHostError),
    #[error(transparent)]
    WorkspaceError(#[from] WorkspaceError),
    #[error(transparent)]
    Sqlx(#[from] SqlxError),
}

impl PrMonitorError {
    fn is_environmental(&self) -> bool {
        matches!(
            self,
            PrMonitorError::GitHostError(
                GitHostError::CliNotInstalled { .. } | GitHostError::NotAGitRepository(_)
            )
        )
    }
}

/// Service to monitor PRs and update task status when they are merged
pub struct PrMonitorService<C: ContainerService> {
    db: DBService,
    poll_interval: Duration,
    analytics: Option<AnalyticsContext>,
    container: C,
    remote_client: Option<RemoteClient>,
    sync_notify: Arc<Notify>,
}

impl<C: ContainerService + Send + Sync + 'static> PrMonitorService<C> {
    pub async fn spawn(
        db: DBService,
        analytics: Option<AnalyticsContext>,
        container: C,
        remote_client: Option<RemoteClient>,
        sync_notify: Arc<Notify>,
    ) -> tokio::task::JoinHandle<()> {
        let service = Self {
            db,
            poll_interval: Duration::from_secs(60),
            analytics,
            container,
            remote_client,
            sync_notify,
        };
        tokio::spawn(async move {
            service.start().await;
        })
    }

    async fn start(&self) {
        info!(
            "Starting PR monitoring service with interval {:?}",
            self.poll_interval
        );

        let mut interval = interval(self.poll_interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.check_all_open_prs().await {
                        error!("Error checking open PRs: {}", e);
                    }
                }
                _ = self.sync_notify.notified() => {
                    debug!("PR sync triggered externally");
                }
            }
            self.sync_pending_to_remote().await;
            match Workspace::pending_archive_workspaces(&self.db.pool).await {
                Ok(ids) => {
                    for id in ids {
                        // A remote operator may have withdrawn this event while
                        // local activity held it. Never replay stale remote intent.
                        let events = match Workspace::archive_provenance(&self.db.pool, id).await {
                            Ok(events) => events,
                            Err(error) => {
                                warn!(%id, ?error, "Failed to read archive provenance");
                                continue;
                            }
                        };
                        let remote_events: Vec<_> = events
                            .iter()
                            .filter(|event| {
                                !event.acknowledged && is_remote_archive_rule(&event.rule)
                            })
                            .collect();
                        let mut allow_remote_intents = false;
                        if !remote_events.is_empty()
                            && let Some(client) = &self.remote_client
                        {
                            match client.get_workspace_archive_intent(id).await {
                                Ok(current) => {
                                    // Register a replacement before retiring its predecessor,
                                    // so the workspace remains in the durable retry set.
                                    allow_remote_intents = match &current {
                                        Some(event) => match Workspace::record_archive_intent(
                                            &self.db.pool,
                                            id,
                                            &event.trigger_key,
                                            &event.rule,
                                            event.triggered_at,
                                        )
                                        .await
                                        {
                                            Ok(()) => true,
                                            Err(error) => {
                                                warn!(%id, ?error, "Failed to persist replacement archive intent");
                                                false
                                            }
                                        },
                                        None => true,
                                    };
                                    if allow_remote_intents {
                                        let current_key = current
                                            .as_ref()
                                            .map(|event| event.trigger_key.as_str());
                                        for event in remote_events {
                                            if current_key != Some(event.trigger_key.as_str())
                                                && let Err(error) =
                                                    Workspace::acknowledge_archive_intent(
                                                        &self.db.pool,
                                                        id,
                                                        &event.trigger_key,
                                                    )
                                                    .await
                                            {
                                                warn!(%id, ?error, "Failed to withdraw archive intent");
                                                allow_remote_intents = false;
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    warn!(%id, ?error, "Remote archive validation deferred; local intents remain eligible")
                                }
                            }
                        }
                        if let Err(error) = self
                            .container
                            .archive_workspace_with_remote_intents(id, allow_remote_intents)
                            .await
                        {
                            warn!(%id, ?error, "Deferred automatic archive failed");
                        }
                    }
                }
                Err(error) => warn!(?error, "Failed to read deferred archive intents"),
            }
        }
    }

    /// Check all open PRs for updates
    async fn check_all_open_prs(&self) -> Result<(), PrMonitorError> {
        let open_prs = PullRequest::get_open(&self.db.pool).await?;

        if open_prs.is_empty() {
            debug!("No open PRs to check");
            return Ok(());
        }

        info!("Checking {} open PRs", open_prs.len());
        for pr in &open_prs {
            if let Err(e) = self.check_open_pr(pr).await {
                if e.is_environmental() {
                    warn!(
                        "Skipping PR #{} due to environmental error: {}",
                        pr.pr_number, e
                    );
                } else {
                    error!("Error checking PR #{}: {}", pr.pr_number, e);
                }
            }
        }

        Ok(())
    }

    /// Check the status of a single open PR and handle state changes.
    async fn check_open_pr(&self, pr: &PullRequest) -> Result<(), PrMonitorError> {
        let credentials =
            github_credentials::resolve_for_urls(&self.db.pool, [pr.pr_url.as_str()]).await;
        let git_host = GitHostService::from_url_with_credentials(&pr.pr_url, credentials)?;
        let status = git_host.get_pr_status(&pr.pr_url).await?;

        debug!(
            "PR #{} status: {:?} (was open)",
            pr.pr_number, status.status
        );

        if matches!(&status.status, MergeStatus::Open) {
            return Ok(());
        }

        let merged_at = if matches!(&status.status, MergeStatus::Merged) {
            Some(status.merged_at.unwrap_or_else(Utc::now))
        } else {
            None
        };

        PullRequest::update_status(
            &self.db.pool,
            &pr.pr_url,
            &status.status,
            merged_at,
            status.merge_commit_sha.clone(),
        )
        .await?;

        // If this is a workspace PR and it was merged, try to archive
        if matches!(&status.status, MergeStatus::Merged)
            && let Some(workspace_id) = pr.workspace_id
        {
            self.try_archive_workspace(
                workspace_id,
                pr.pr_number,
                &pr.pr_url,
                merged_at.unwrap_or_else(Utc::now),
            )
            .await?;
        }

        info!("PR #{} status changed to {:?}", pr.pr_number, status.status);

        Ok(())
    }

    /// Archive workspace if all its PRs are merged/closed
    async fn try_archive_workspace(
        &self,
        workspace_id: uuid::Uuid,
        pr_number: i64,
        pr_url: &str,
        merged_at: chrono::DateTime<Utc>,
    ) -> Result<(), PrMonitorError> {
        let Some(workspace) = Workspace::find_by_id(&self.db.pool, workspace_id).await? else {
            return Ok(());
        };

        let open_pr_count =
            PullRequest::count_open_for_workspace(&self.db.pool, workspace_id).await?;

        if open_pr_count == 0 {
            Workspace::record_archive_intent(
                &self.db.pool,
                workspace_id,
                &format!("pr:{pr_url}"),
                "pr_merged",
                merged_at,
            )
            .await?;
            info!(
                "PR #{} was merged, archiving workspace {}",
                pr_number, workspace.id
            );
            if let Err(e) = self
                .container
                .archive_workspace_automatically(workspace.id)
                .await
            {
                error!("Failed to archive workspace {}: {}", workspace.id, e);
            }

            if let Some(analytics) = &self.analytics {
                analytics.analytics_service.track_event(
                    &analytics.user_id,
                    "pr_merged",
                    Some(json!({
                        "workspace_id": workspace.id.to_string(),
                    })),
                );
            }
        } else {
            info!(
                "PR #{} was merged, leaving workspace {} active with {} open PR(s)",
                pr_number, workspace.id, open_pr_count
            );
        }

        Ok(())
    }

    /// Sync pending PR status changes to remote server.
    async fn sync_pending_to_remote(&self) {
        let Some(client) = &self.remote_client else {
            return;
        };

        let pending = match PullRequest::get_pending_sync(&self.db.pool).await {
            Ok(prs) => prs,
            Err(e) => {
                error!("Failed to query pending sync PRs: {}", e);
                return;
            }
        };

        if pending.is_empty() {
            return;
        }

        debug!("Syncing {} pending PRs to remote", pending.len());

        for pr in &pending {
            let pr_api_status = match &pr.pr_status {
                MergeStatus::Open => PullRequestStatus::Open,
                MergeStatus::Merged => PullRequestStatus::Merged,
                MergeStatus::Closed => PullRequestStatus::Closed,
                MergeStatus::Unknown => continue,
            };

            let request = UpdatePullRequestApiRequest {
                url: pr.pr_url.clone(),
                status: Some(pr_api_status),
                merged_at: pr.merged_at.map(Some),
                merge_commit_sha: pr.merge_commit_sha.clone().map(Some),
            };

            match client.update_pull_request(request).await {
                Ok(_) => {
                    if let Err(e) = PullRequest::mark_synced(&self.db.pool, &pr.id).await {
                        error!("Failed to mark PR #{} as synced: {}", pr.pr_number, e);
                    }
                }
                Err(RemoteClientError::Http { status: 404, .. }) => {
                    if let Some(workspace_id) = pr.workspace_id {
                        let request = UpsertPullRequestRequest {
                            url: pr.pr_url.clone(),
                            number: pr.pr_number as i32,
                            status: pr_api_status,
                            merged_at: pr.merged_at,
                            merge_commit_sha: pr.merge_commit_sha.clone(),
                            target_branch_name: pr.target_branch_name.clone(),
                            local_workspace_id: workspace_id,
                        };
                        remote_sync::sync_pr_to_remote(client, request).await;
                        if let Err(e) = PullRequest::mark_synced(&self.db.pool, &pr.id).await {
                            error!("Failed to mark PR #{} as synced: {}", pr.pr_number, e);
                        }
                    } else {
                        warn!(
                            "PR #{} not found on remote and has no workspace, removing local record",
                            pr.pr_number
                        );
                        if let Err(e) = PullRequest::delete(&self.db.pool, &pr.id).await {
                            error!("Failed to delete orphaned local PR: {}", e);
                        }
                    }
                }
                Err(RemoteClientError::Auth) => {
                    debug!("PR sync sweep stopped: not authenticated");
                    return;
                }
                Err(e) => {
                    error!(
                        "Failed to sync PR #{} status to remote: {}",
                        pr.pr_number, e
                    );
                }
            }
        }
    }
}

fn is_remote_archive_rule(rule: &str) -> bool {
    rule.starts_with("issue_") || rule.starts_with("remote_") || rule == "legacy_remote_archive"
}

#[cfg(test)]
mod archive_rule_tests {
    #[test]
    fn only_remote_events_need_remote_revalidation() {
        for rule in [
            "issue_done",
            "issue_cancelled",
            "remote_archive",
            "legacy_remote_archive",
        ] {
            assert!(super::is_remote_archive_rule(rule));
        }
        for rule in [
            "pr_merged",
            "local_merge",
            "manual_archive",
            "legacy_archive_unknown",
        ] {
            assert!(!super::is_remote_archive_rule(rule));
        }
    }
}
