//! The Jira <-> VK reconciler.
//!
//! One global ticker scans for due configs (interval elapsed or "sync now"
//! requested) and runs a pass per config. Passes are level-triggered: a
//! crashed pass leaves `last_sync_completed_at` stale and is retried on the
//! next tick. Per-issue failures never abort a pass (FR-17); they are
//! recorded on the link row and aggregated into the config's
//! `last_sync_error` (FR-16).

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tokio::task::JoinHandle;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use super::{
    client::{JiraClient, JiraClientError, JiraIssueData},
    mapping::{resolve_jira_to_vk, resolve_vk_to_jira, seed_vk_to_jira},
    merge::{FieldAction, decide_field},
    types::{
        JiraAuthMode, JiraStatusMapping, JiraSyncConfig, LINK_STATE_ACTIVE,
        LINK_STATE_DELETED_REMOTE, LINK_STATE_DORMANT,
    },
};
use crate::{
    auth::JwtService,
    db::{
        issues::IssueRepository,
        jira_sync::{JiraSyncRepository, LinkSnapshot},
        project_statuses::ProjectStatusRepository,
    },
};

const DEFAULT_TICK: Duration = Duration::from_secs(30);

/// Spawn the background reconciler. Call once during server startup.
pub fn spawn_jira_sync_task(
    pool: PgPool,
    http: reqwest::Client,
    jwt: Arc<JwtService>,
) -> JoinHandle<()> {
    let tick = std::env::var("JIRA_SYNC_TICK_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_TICK);

    info!(
        tick_secs = tick.as_secs(),
        "Starting Jira sync background task"
    );

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(tick);
        // Skip the immediate first tick so the server can finish starting up.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            run_tick(&pool, &http, &jwt).await;
        }
    })
}

async fn run_tick(pool: &PgPool, http: &reqwest::Client, jwt: &JwtService) {
    let due = match JiraSyncRepository::list_due_config_ids(pool).await {
        Ok(due) => due,
        Err(error) => {
            warn!(?error, "jira sync: failed to list due configs");
            return;
        }
    };

    for config_id in due {
        // The claim is the authoritative check: it atomically re-evaluates
        // enabled/dueness/lease and returns the row the pass must use, so a
        // disable or edit racing this tick takes effect immediately.
        let config = match JiraSyncRepository::claim_due_config(pool, config_id).await {
            Ok(Some(config)) => config,
            Ok(None) => continue,
            Err(error) => {
                warn!(?error, %config_id, "jira sync: failed to claim config");
                continue;
            }
        };
        run_pass(pool, http, jwt, config).await;
    }
}

#[derive(Debug, Default)]
struct PassStats {
    created: u32,
    updated_vk: u32,
    updated_jira: u32,
    errors: Vec<String>,
}

impl PassStats {
    fn error_summary(&self) -> Option<String> {
        match self.errors.as_slice() {
            [] => None,
            [only] => Some(only.clone()),
            [first, ..] => Some(format!(
                "{} issues failed to sync; first: {first}",
                self.errors.len()
            )),
        }
    }
}

/// Run one claimed pass. `config` must come from
/// [`JiraSyncRepository::claim_due_config`]; its `last_sync_started_at` is
/// the lease token guarding completion.
#[instrument(name = "jira_sync.pass", skip_all, fields(project_id = %config.project_id))]
async fn run_pass(pool: &PgPool, http: &reqwest::Client, jwt: &JwtService, config: JiraSyncConfig) {
    let Some(lease) = config.last_sync_started_at else {
        warn!("jira sync: claimed config has no lease timestamp");
        return;
    };

    let outcome = do_pass(pool, http, jwt, &config).await;

    let error_text = match outcome {
        Ok(stats) => {
            info!(
                created = stats.created,
                updated_vk = stats.updated_vk,
                updated_jira = stats.updated_jira,
                errors = stats.errors.len(),
                "jira sync: pass complete"
            );
            stats.error_summary()
        }
        Err(error) => {
            warn!(%error, "jira sync: pass aborted");
            Some(error.to_string())
        }
    };

    if let Err(error) =
        JiraSyncRepository::mark_sync_completed(pool, config.id, lease, error_text).await
    {
        warn!(?error, "jira sync: failed to mark pass completed");
    }
}

#[derive(Debug, thiserror::Error)]
enum PassError {
    #[error("{0}")]
    Jira(#[from] JiraClientError),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("database error")]
    Db(#[from] crate::db::jira_sync::JiraSyncDbError),
    #[error("database error")]
    Sqlx(#[from] sqlx::Error),
}

impl From<crate::db::project_statuses::ProjectStatusError> for PassError {
    fn from(err: crate::db::project_statuses::ProjectStatusError) -> Self {
        match err {
            crate::db::project_statuses::ProjectStatusError::Database(e) => PassError::Sqlx(e),
        }
    }
}

/// Project status lookup tables for mapping between names and ids.
struct StatusIndex {
    name_by_id: HashMap<Uuid, String>,
    id_by_lower_name: HashMap<String, Uuid>,
    /// First visible column by sort order — the landing spot when a Jira
    /// status can't be mapped (e.g. brand-new issue with an unknown status).
    default_status_id: Option<Uuid>,
}

impl StatusIndex {
    fn id_for_name(&self, name: &str) -> Option<Uuid> {
        self.id_by_lower_name.get(&name.to_lowercase()).copied()
    }
}

async fn do_pass(
    pool: &PgPool,
    http: &reqwest::Client,
    jwt: &JwtService,
    config: &JiraSyncConfig,
) -> Result<PassStats, PassError> {
    let auth_mode = JiraAuthMode::parse(&config.auth_mode)
        .ok_or_else(|| PassError::Config(format!("unknown auth mode {}", config.auth_mode)))?;
    let credential = jwt
        .decrypt_string(&config.encrypted_credential)
        .map_err(|_| PassError::Config("failed to decrypt stored credential".to_string()))?;
    let client = JiraClient::new(
        http.clone(),
        &config.jira_base_url,
        auth_mode,
        config.jira_email.clone(),
        credential,
    )?;

    let search = client.search_all(&config.jql).await?;
    let jira_issues = search.issues;

    let statuses = ProjectStatusRepository::list_by_project(pool, config.project_id).await?;
    let mut sorted = statuses.clone();
    sorted.sort_by_key(|s| s.sort_order);
    let status_index = StatusIndex {
        name_by_id: statuses.iter().map(|s| (s.id, s.name.clone())).collect(),
        id_by_lower_name: statuses
            .iter()
            .map(|s| (s.name.to_lowercase(), s.id))
            .collect(),
        default_status_id: sorted
            .iter()
            .find(|s| !s.hidden)
            .or(sorted.first())
            .map(|s| s.id),
    };

    // Seed reverse status mapping from the statuses observed in this query,
    // so VK -> Jira transitions work out of the box (FR-13).
    let mut mapping = config.parsed_status_mapping();
    let observed: Vec<(String, Option<String>)> = jira_issues
        .iter()
        .map(|i| (i.status_name.clone(), i.status_category.clone()))
        .collect();
    if seed_vk_to_jira(&mut mapping, &observed)
        && let Ok(value) = serde_json::to_value(&mapping)
    {
        JiraSyncRepository::update_status_mapping(pool, config.id, value).await?;
    }

    let links = JiraSyncRepository::list_links_by_project(pool, config.project_id).await?;
    let links_by_jira_id: HashMap<&str, &crate::jira::types::JiraIssueLink> = links
        .iter()
        .map(|l| (l.jira_issue_id.as_str(), l))
        .collect();

    let mut stats = PassStats::default();

    for jira_issue in &jira_issues {
        let result = match links_by_jira_id.get(jira_issue.id.as_str()) {
            None => import_new_issue(pool, config, &status_index, &mapping, &client, jira_issue)
                .await
                .map(|()| {
                    stats.created += 1;
                }),
            Some(link) if link.link_state == LINK_STATE_DELETED_REMOTE => continue,
            Some(link) => {
                sync_linked_issue(
                    pool,
                    &client,
                    &mapping,
                    &status_index,
                    link,
                    jira_issue,
                    &mut stats,
                )
                .await
            }
        };
        if let Err(error) = result {
            let message = format!("{}: {error}", jira_issue.key);
            warn!(%message, "jira sync: issue failed");
            if let Some(link) = links_by_jira_id.get(jira_issue.id.as_str()) {
                let _ = JiraSyncRepository::set_link_error(pool, link.id, Some(error.to_string()))
                    .await;
            }
            stats.errors.push(message);
        }
    }

    // Scope-out detection (FR-9): active links whose issue was not returned
    // by the query either left the JQL scope (dormant) or were deleted in
    // Jira (permanently unlinked). VK issues are never deleted. Skipped
    // entirely when the search was truncated by the page cap — a partial
    // result set must not be mistaken for the full JQL scope.
    if search.truncated {
        stats.errors.push(format!(
            "JQL matched more issues than the sync cap; synced the first {}, \
             skipped out-of-scope detection — narrow the query",
            jira_issues.len()
        ));
    } else {
        let returned: std::collections::HashSet<&str> =
            jira_issues.iter().map(|i| i.id.as_str()).collect();
        for link in links.iter().filter(|l| {
            l.link_state == LINK_STATE_ACTIVE && !returned.contains(l.jira_issue_id.as_str())
        }) {
            match client.get_issue(&link.jira_issue_id).await {
                Ok(Some(_)) => {
                    JiraSyncRepository::set_link_state(pool, link.id, LINK_STATE_DORMANT).await?;
                }
                Ok(None) => {
                    JiraSyncRepository::set_link_state(pool, link.id, LINK_STATE_DELETED_REMOTE)
                        .await?;
                }
                Err(error) => {
                    stats.errors.push(format!(
                        "{}: scope check failed: {error}",
                        link.jira_issue_key
                    ));
                }
            }
        }
    }

    Ok(stats)
}

/// First sight of a Jira issue: create the VK issue and its link row.
///
/// `IssueRepository::create` runs its own transaction (the simple-id trigger
/// lives there), so issue and link cannot be created atomically. The issue's
/// `extension_metadata` carries its Jira identity, and
/// `find_issue_id_by_jira_metadata` re-links such an orphan on the next pass
/// instead of duplicating it.
async fn import_new_issue(
    pool: &PgPool,
    config: &JiraSyncConfig,
    status_index: &StatusIndex,
    mapping: &JiraStatusMapping,
    client: &JiraClient,
    jira_issue: &JiraIssueData,
) -> Result<(), PassError> {
    let status_id = resolve_jira_to_vk(
        mapping,
        &jira_issue.status_name,
        jira_issue.status_category.as_deref(),
    )
    .and_then(|name| status_index.id_for_name(&name))
    .or(status_index.default_status_id)
    .ok_or_else(|| PassError::Config("project has no statuses".to_string()))?;

    let description = normalize_description(jira_issue.description.clone());

    let issue_id = match JiraSyncRepository::find_issue_id_by_jira_metadata(
        pool,
        config.project_id,
        &jira_issue.id,
    )
    .await?
    {
        Some(orphan_id) => orphan_id,
        None => {
            let creator = config.created_by_user_id.ok_or_else(|| {
                PassError::Config(
                    "sync config has no owning user to attribute created issues to".to_string(),
                )
            })?;
            let sort_order = JiraSyncRepository::next_sort_order(pool, config.project_id).await?;
            let created = IssueRepository::create(
                pool,
                None,
                config.project_id,
                status_id,
                jira_issue.summary.clone(),
                description.clone(),
                None,
                None,
                None,
                None,
                sort_order,
                None,
                None,
                serde_json::json!({
                    "jira": { "issue_id": jira_issue.id, "issue_key": jira_issue.key }
                }),
                creator,
            )
            .await
            .map_err(|e| PassError::Config(format!("failed to create issue: {e}")))?;
            created.data.id
        }
    };

    let vk_issue = IssueRepository::find_by_id(pool, issue_id)
        .await
        .map_err(|e| PassError::Config(format!("failed to load created issue: {e}")))?
        .ok_or_else(|| PassError::Config("created issue vanished".to_string()))?;

    let snapshot = LinkSnapshot {
        title: jira_issue.summary.clone(),
        description,
        status_id: vk_issue.status_id,
        jira_status: jira_issue.status_name.clone(),
        jira_updated_at: jira_issue.updated,
        vk_updated_at: vk_issue.updated_at,
    };
    JiraSyncRepository::create_link(
        pool,
        config.id,
        config.project_id,
        issue_id,
        &jira_issue.id,
        &jira_issue.key,
        &client.browse_url(&jira_issue.key),
        &snapshot,
    )
    .await?;
    Ok(())
}

/// Empty and missing descriptions are equivalent on both sides; normalizing
/// avoids ping-pong writes over "" vs NULL.
fn normalize_description(description: Option<String>) -> Option<String> {
    description.filter(|d| !d.trim().is_empty())
}

#[allow(clippy::too_many_arguments)]
async fn sync_linked_issue(
    pool: &PgPool,
    client: &JiraClient,
    mapping: &JiraStatusMapping,
    status_index: &StatusIndex,
    link: &crate::jira::types::JiraIssueLink,
    jira_issue: &JiraIssueData,
    stats: &mut PassStats,
) -> Result<(), PassError> {
    let Some(vk_issue) = IssueRepository::find_by_id(pool, link.issue_id)
        .await
        .map_err(|e| PassError::Config(format!("failed to load issue: {e}")))?
    else {
        // The VK issue was deleted by a user. Drop the link; the issue still
        // matches the JQL, so the next pass re-imports it (FR-5).
        sqlx::query!("DELETE FROM jira_issue_links WHERE id = $1", link.id)
            .execute(pool)
            .await?;
        return Ok(());
    };

    // --- Decide per field against the snapshot (3-way merge). ---
    let jira_title = jira_issue.summary.clone();
    let vk_title = vk_issue.title.clone();
    let snap_title = link.last_synced_title.clone().unwrap_or_default();

    let jira_desc = normalize_description(jira_issue.description.clone());
    let vk_desc = normalize_description(vk_issue.description.clone());
    let snap_desc = normalize_description(link.last_synced_description.clone());

    let jira_status = jira_issue.status_name.clone();
    let snap_jira_status = link.last_synced_jira_status.clone().unwrap_or_default();
    let vk_status_id = vk_issue.status_id;
    let snap_status_id = link.last_synced_status_id;

    let title_action = field_action(
        jira_title == snap_title,
        vk_title == snap_title,
        &jira_title,
        &vk_title,
        jira_issue.updated,
        vk_issue.updated_at,
    );
    let desc_action = field_action(
        jira_desc == snap_desc,
        vk_desc == snap_desc,
        &jira_desc,
        &vk_desc,
        jira_issue.updated,
        vk_issue.updated_at,
    );
    // Status equality across systems means "VK status equals the mapped Jira
    // status"; comparing each side to its own snapshot sidesteps mapping
    // ambiguity, and the converged fast-path checks the mapped target below.
    let status_action = decide_status_action(
        &jira_status,
        jira_issue.status_category.as_deref(),
        &snap_jira_status,
        vk_status_id,
        snap_status_id,
        jira_issue.updated,
        vk_issue.updated_at,
        mapping,
        status_index,
    );

    let nothing_to_do = title_action == FieldAction::NoOp
        && desc_action == FieldAction::NoOp
        && status_action == FieldAction::NoOp;
    if nothing_to_do && link.link_state == LINK_STATE_ACTIVE && link.last_error.is_none() {
        return Ok(());
    }

    // --- Outbound (VK -> Jira) writes first. ---
    let mut wrote_jira = false;
    let out_summary = (title_action == FieldAction::WriteJira).then_some(vk_title.as_str());
    let out_description = (desc_action == FieldAction::WriteJira).then_some(vk_desc.as_deref());
    if out_summary.is_some() || out_description.is_some() {
        client
            .update_issue_fields(&link.jira_issue_id, out_summary, out_description)
            .await?;
        wrote_jira = true;
    }

    let mut status_push_error: Option<String> = None;
    if status_action == FieldAction::WriteJira {
        let vk_status_name = status_index
            .name_by_id
            .get(&vk_status_id)
            .cloned()
            .unwrap_or_default();
        match resolve_vk_to_jira(mapping, &vk_status_name) {
            Some(target) if !target.eq_ignore_ascii_case(&jira_status) => {
                match client
                    .transition_to_status(&link.jira_issue_id, &target)
                    .await
                {
                    Ok(()) => wrote_jira = true,
                    Err(error) => {
                        status_push_error = Some(format!(
                            "status \"{vk_status_name}\" not propagated: {error}"
                        ));
                    }
                }
            }
            Some(_) => {} // already at the mapped target
            None => {
                status_push_error = Some(format!(
                    "status \"{vk_status_name}\" has no Jira mapping; change not propagated"
                ));
            }
        }
    }

    // Re-read Jira after writing so the snapshot records Jira's own
    // (possibly normalized) representation — otherwise the next pass would
    // see a phantom Jira-side change and write it back to VK.
    let jira_now: JiraIssueData = if wrote_jira {
        client
            .get_issue(&link.jira_issue_id)
            .await?
            .ok_or(JiraClientError::NotFound)?
    } else {
        jira_issue.clone()
    };
    if wrote_jira {
        stats.updated_jira += 1;
    }

    // --- Inbound (Jira -> VK) writes + snapshot, atomically. ---
    let new_title = (title_action == FieldAction::WriteVk).then(|| jira_now.summary.clone());
    let new_desc = (desc_action == FieldAction::WriteVk)
        .then(|| normalize_description(jira_now.description.clone()));
    let new_status_id = if status_action == FieldAction::WriteVk {
        resolve_jira_to_vk(
            mapping,
            &jira_now.status_name,
            jira_now.status_category.as_deref(),
        )
        .and_then(|name| status_index.id_for_name(&name))
    } else {
        None
    };

    let mut tx = pool.begin().await?;
    let final_issue = if new_title.is_some() || new_desc.is_some() || new_status_id.is_some() {
        let updated = IssueRepository::update(
            &mut *tx,
            vk_issue.id,
            new_status_id,
            new_title,
            new_desc,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(|e| PassError::Config(format!("failed to update issue: {e}")))?;
        stats.updated_vk += 1;
        updated
    } else {
        vk_issue
    };

    // When the status push failed, keep the old status snapshot so the next
    // pass retries (and keeps the failure visible) instead of recording the
    // divergence as converged.
    let (snap_status_id_new, snap_jira_status_new) = if status_push_error.is_some() {
        (
            snap_status_id.unwrap_or(final_issue.status_id),
            snap_jira_status.clone(),
        )
    } else {
        (final_issue.status_id, jira_now.status_name.clone())
    };

    let snapshot = LinkSnapshot {
        title: final_issue.title.clone(),
        description: normalize_description(final_issue.description.clone()),
        status_id: snap_status_id_new,
        jira_status: snap_jira_status_new,
        jira_updated_at: jira_now.updated,
        vk_updated_at: final_issue.updated_at,
    };
    JiraSyncRepository::update_link_snapshot(
        &mut *tx,
        link.id,
        &jira_now.key,
        &client.browse_url(&jira_now.key),
        &snapshot,
    )
    .await?;
    tx.commit().await?;

    if let Some(message) = status_push_error {
        JiraSyncRepository::set_link_error(pool, link.id, Some(message.clone())).await?;
        return Err(PassError::Config(message));
    }

    Ok(())
}

/// `decide_field` plus the "both sides already hold the same value" fast
/// path, which is convergence regardless of what the snapshot says.
fn field_action<T: PartialEq>(
    jira_matches_snapshot: bool,
    vk_matches_snapshot: bool,
    jira_value: &T,
    vk_value: &T,
    jira_updated: Option<DateTime<Utc>>,
    vk_updated: DateTime<Utc>,
) -> FieldAction {
    if jira_value == vk_value {
        return FieldAction::NoOp;
    }
    decide_field(
        jira_matches_snapshot,
        vk_matches_snapshot,
        jira_updated,
        vk_updated,
    )
}

#[allow(clippy::too_many_arguments)]
fn decide_status_action(
    jira_status: &str,
    jira_category: Option<&str>,
    snap_jira_status: &str,
    vk_status_id: Uuid,
    snap_status_id: Option<Uuid>,
    jira_updated: Option<DateTime<Utc>>,
    vk_updated: DateTime<Utc>,
    mapping: &JiraStatusMapping,
    status_index: &StatusIndex,
) -> FieldAction {
    // Converged fast path: VK already sits on the column the Jira status maps to.
    let mapped_vk_id = resolve_jira_to_vk(mapping, jira_status, jira_category)
        .and_then(|name| status_index.id_for_name(&name));
    let jira_changed = !jira_status.eq_ignore_ascii_case(snap_jira_status);
    let vk_changed = snap_status_id != Some(vk_status_id);
    if mapped_vk_id == Some(vk_status_id) && !jira_changed {
        return FieldAction::NoOp;
    }
    decide_field(!jira_changed, !vk_changed, jira_updated, vk_updated)
}
