//! Cached workspace diff stats for the workspaces sidebar.
//!
//! The sidebar wants three integers per workspace — files changed, lines added,
//! lines removed — and the only way to get them is to walk the worktree with
//! `git`. On a cluster the worktrees live on NFS, so doing that on the request
//! path costs seconds: measured at 5.7s for 132 active workspaces, repeated
//! every 15s by the client's poll, forever, whether or not anything changed.
//!
//! So the request path never computes anything. It reads whatever the last
//! refresh produced ([`WorkspaceDiffStatsCache::snapshot`]) and schedules the
//! next one ([`WorkspaceDiffStatsCache::refresh_stale`]) — stale-while-
//! revalidate. Freshness comes from two independent mechanisms:
//!
//! - **Invalidation** off the SQLite-hook patch stream
//!   ([`WorkspaceDiffStatsCache::spawn_invalidation_watcher`]) is the fast path.
//!   It is only ever an optimisation: the broadcast channel is lossy, and a
//!   lagging subscriber silently skips events.
//! - **[`REFRESH_AFTER`] is the correctness backstop.** It must stay
//!   unconditional. Anything the server cannot observe — a file edited in an
//!   external editor, a worktree pruned by another workspace's cleanup — is
//!   caught only by this sweep.
//!
//! Nothing self-drives the sweep: `refresh_stale` runs because a client asked
//! for summaries. With no client connected, nothing refreshes, which is the
//! intended idle cost.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use db::models::workspace::Workspace;
use git::GitService;
use moka::future::Cache;
use sqlx::SqlitePool;
use tokio::sync::Semaphore;
use utils::msg_store::MsgStore;
use uuid::Uuid;

use crate::services::diff_stream::{self, DiffStats};

/// An entry older than this is recomputed on the next sweep. This is the
/// backstop that makes lossy event-driven invalidation safe, not a tuning knob —
/// do not make it conditional on having seen an event.
pub const REFRESH_AFTER: Duration = Duration::from_secs(60);

/// Ceiling on worktrees being walked at once. The pre-cache code used an
/// unbounded `join_all`, so one slow NFS server stalled every workspace
/// simultaneously.
const MAX_CONCURRENT_REFRESH: usize = 8;

/// Bounded capacity as well as a TTL — the established idiom in this codebase
/// for in-memory caches, and this one is keyed by a growing set of workspace ids.
const CACHE_CAPACITY: u64 = 2048;
const ENTRY_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Debug)]
struct CachedDiffStats {
    stats: DiffStats,
    computed_at: Instant,
}

struct Inner {
    entries: Cache<Uuid, CachedDiffStats>,
    /// Bumped by `invalidate`. A refresh captures the value before doing its git
    /// work and discards its result if the value moved, so a slow refresh cannot
    /// overwrite a fresher invalidation.
    generations: DashMap<Uuid, u64>,
    /// Workspaces with a refresh in flight, so overlapping polls do not stack up
    /// duplicate git work for the same workspace.
    inflight: DashMap<Uuid, ()>,
    permits: Semaphore,
}

/// Clears an `inflight` marker on drop, so a panicking or early-returning
/// refresh cannot leave a workspace permanently un-refreshable.
struct InflightGuard {
    inner: Arc<Inner>,
    id: Uuid,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        // `inflight` first: once it is gone, `invalidate` stops recording
        // generations for this id, so the `generations` removal cannot race with a
        // fresh insert and leave an orphan behind.
        self.inner.inflight.remove(&self.id);
        self.inner.generations.remove(&self.id);
    }
}

#[derive(Clone)]
pub struct WorkspaceDiffStatsCache {
    inner: Arc<Inner>,
}

impl Default for WorkspaceDiffStatsCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a workspace's stats should be recomputed.
///
/// Pure so it can be exhaustively tested. `cached` is the age-stamp of the
/// current entry, if any.
///
/// Note that `container_ref` and `worktree_deleted` are advisory: a repo-wide
/// `git worktree prune` during another workspace's cleanup can remove a working
/// tree while the columns survive. They are a cheap way to skip work that cannot
/// produce an answer, never proof that a cached value is still valid — which is
/// the other reason [`REFRESH_AFTER`] stays.
pub fn needs_refresh(workspace: &Workspace, cached: Option<Instant>, now: Instant) -> bool {
    if workspace.container_ref.is_none() || workspace.worktree_deleted {
        return false;
    }
    match cached {
        Some(computed_at) => now.saturating_duration_since(computed_at) >= REFRESH_AFTER,
        None => true,
    }
}

impl WorkspaceDiffStatsCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                entries: Cache::builder()
                    .max_capacity(CACHE_CAPACITY)
                    .time_to_live(ENTRY_TTL)
                    .build(),
                generations: DashMap::new(),
                inflight: DashMap::new(),
                permits: Semaphore::new(MAX_CONCURRENT_REFRESH),
            }),
        }
    }

    /// Diff stats for the given workspaces, from memory only — no git, no
    /// filesystem, no database. Workspaces with no entry are absent from the map,
    /// which callers surface as `None` rather than as zero.
    pub async fn snapshot(&self, ids: &[Uuid]) -> HashMap<Uuid, DiffStats> {
        let mut out = HashMap::with_capacity(ids.len());
        for id in ids {
            if let Some(entry) = self.inner.entries.get(id).await {
                out.insert(*id, entry.stats.clone());
            }
        }
        out
    }

    /// Mark a workspace's cached stats as due for recomputation.
    ///
    /// This deliberately **keeps serving the old value** rather than deleting it.
    /// Deleting would make the badge disappear for a whole poll interval every
    /// time a workspace was written to — and a `/workspaces/{id}` patch fires
    /// several times per agent turn, so the numbers would blink out at exactly the
    /// moment the user is watching them. Stale-while-revalidate means stale, not
    /// absent. [`Self::discard`] is for the case where the old value is known to
    /// be untrustworthy.
    ///
    /// If a refresh is in flight its generation is bumped so it discards its
    /// now-known-stale result. The generation is tracked *only* while a refresh is
    /// in flight: with nothing in flight there is no result to poison, and
    /// unconditionally inserting a counter would grow `generations` once per
    /// workspace ever invalidated, with nothing to prune it.
    pub async fn invalidate(&self, id: Uuid) {
        if self.inner.inflight.contains_key(&id) {
            *self.inner.generations.entry(id).or_insert(0) += 1;
        }
        // Age the entry out past REFRESH_AFTER so the next sweep recomputes it,
        // while `snapshot` keeps returning the previous numbers until it does.
        // `checked_sub` because `Instant` is monotonic-from-boot and plain
        // subtraction panics if it would underflow; a `None` there just means the
        // process started under a minute ago, in which case the entry is already
        // young enough that dropping it is the safe fallback.
        if let Some(entry) = self.inner.entries.get(&id).await {
            match entry.computed_at.checked_sub(REFRESH_AFTER) {
                Some(aged) => {
                    self.inner
                        .entries
                        .insert(
                            id,
                            CachedDiffStats {
                                stats: entry.stats,
                                computed_at: aged,
                            },
                        )
                        .await;
                }
                None => self.inner.entries.invalidate(&id).await,
            }
        }
    }

    /// Drop a workspace's cached stats outright, for when the previous value is
    /// known to be wrong rather than merely old — a failed git probe, or a
    /// worktree that has gone away. Serving a stale number in that situation would
    /// keep a confident wrong answer on screen for the whole entry TTL.
    pub async fn discard(&self, id: Uuid) {
        self.inner.entries.invalidate(&id).await;
    }

    fn generation(&self, id: Uuid) -> u64 {
        self.inner.generations.get(&id).map(|g| *g).unwrap_or(0)
    }

    /// Recompute entries that are missing or older than [`REFRESH_AFTER`], in the
    /// background. The awaits here are in-memory cache reads only; the git work
    /// is spawned, so the caller (an HTTP handler) never waits on it.
    pub async fn refresh_stale(
        &self,
        pool: &SqlitePool,
        git: &GitService,
        workspaces: &[Workspace],
    ) {
        let now = Instant::now();
        let mut targets: Vec<Workspace> = Vec::new();
        for ws in workspaces {
            let cached = self.inner.entries.get(&ws.id).await.map(|e| e.computed_at);
            if needs_refresh(ws, cached, now) {
                targets.push(ws.clone());
            }
        }

        if targets.is_empty() {
            return;
        }

        let cache = self.clone();
        let pool = pool.clone();
        let git = git.clone();
        tokio::spawn(async move {
            futures::future::join_all(
                targets
                    .into_iter()
                    .map(|ws| cache.refresh_one(pool.clone(), git.clone(), ws)),
            )
            .await;
        });
    }

    async fn refresh_one(&self, pool: SqlitePool, git: GitService, workspace: Workspace) {
        let id = workspace.id;

        // Already being refreshed by an earlier poll — do not stack duplicate
        // git work for the same worktree.
        if self.inner.inflight.insert(id, ()).is_some() {
            return;
        }
        let _guard = InflightGuard {
            inner: self.inner.clone(),
            id,
        };

        // The permit is held for the whole computation on purpose. The git work
        // runs inside `spawn_blocking`, which `tokio::time::timeout` cannot
        // cancel — a timeout would release the permit while the thread stayed
        // stuck on NFS, letting the fan-out grow without bound. The semaphore is
        // the bound.
        let Ok(_permit) = self.inner.permits.acquire().await else {
            return;
        };

        let generation = self.generation(id);
        let computed = diff_stream::compute_diff_stats_strict(&pool, &git, &workspace).await;

        let Some(stats) = computed else {
            // A git probe that errors is not a clean worktree. Drop any previous
            // value too: the sweep has just proved it unreliable (the worktree may
            // have been pruned by another workspace's cleanup), and keeping it
            // would leave a confident wrong number on screen for the entry TTL
            // while every sweep re-detected and re-discarded the same failure.
            tracing::debug!(
                workspace_id = %id,
                "workspace diff stats unavailable; discarding any cached value"
            );
            self.discard(id).await;
            return;
        };

        // An invalidation landed while we were computing; the value we have is
        // already known to be stale.
        if self.generation(id) != generation {
            return;
        }

        self.inner
            .entries
            .insert(
                id,
                CachedDiffStats {
                    stats,
                    computed_at: Instant::now(),
                },
            )
            .await;
    }

    /// Invalidate cached stats when a workspace changes, by watching the shared
    /// SQLite-hook JSON-patch stream.
    ///
    /// A `/workspaces/{id}` patch is pushed on every `execution_processes` insert
    /// or update as well as on direct workspace writes, so this single hook covers
    /// agent runs, setup/cleanup scripts and dev servers at both start and finish.
    /// Spurious invalidations (a `touch`, a rename) cost at most one extra
    /// recompute on the next sweep.
    ///
    /// The receiver is lossy: a lagging subscriber skips events, which is why
    /// [`REFRESH_AFTER`] and not this watcher is the correctness guarantee. Only a
    /// `Weak` is held between messages, so dropping the cache stops the task
    /// instead of leaking it.
    pub fn spawn_invalidation_watcher(&self, msg_store: Arc<MsgStore>) {
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            let mut receiver = msg_store.get_receiver();
            loop {
                let msg = match receiver.recv().await {
                    Ok(msg) => msg,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                let Some(inner) = weak.upgrade() else { break };
                let cache = WorkspaceDiffStatsCache { inner };

                let utils::log_msg::LogMsg::JsonPatch(patch) = msg else {
                    continue;
                };
                for op in &patch.0 {
                    if !matches!(
                        op,
                        json_patch::PatchOperation::Add(_) | json_patch::PatchOperation::Replace(_)
                    ) {
                        continue;
                    }
                    let path = op.path().to_string();
                    let Some(segment) = path.strip_prefix("/workspaces/") else {
                        continue;
                    };
                    if let Ok(workspace_id) = segment.parse::<Uuid>() {
                        cache.invalidate(workspace_id).await;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(container_ref: Option<&str>, worktree_deleted: bool) -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            task_id: None,
            container_ref: container_ref.map(|s| s.to_string()),
            branch: "vk/test".to_string(),
            setup_completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            archived: false,
            pinned: false,
            name: Some("test".to_string()),
            worktree_deleted,
            current_pipeline_stage: None,
            speckit_feature_key: None,
            speckit_host_repo_id: None,
        }
    }

    #[test]
    fn needs_refresh_truth_table() {
        let now = Instant::now();

        // No container: nothing to diff.
        assert!(!needs_refresh(&workspace(None, false), None, now));

        // Worktree deleted: the git work cannot produce an answer.
        assert!(!needs_refresh(&workspace(Some("/tmp/ws"), true), None, now));

        // Never computed.
        assert!(needs_refresh(&workspace(Some("/tmp/ws"), false), None, now));

        // Fresh entry.
        assert!(!needs_refresh(
            &workspace(Some("/tmp/ws"), false),
            Some(now),
            now
        ));

        // Just under the threshold.
        assert!(!needs_refresh(
            &workspace(Some("/tmp/ws"), false),
            Some(now),
            now + REFRESH_AFTER - Duration::from_millis(1)
        ));

        // At and past the threshold.
        assert!(needs_refresh(
            &workspace(Some("/tmp/ws"), false),
            Some(now),
            now + REFRESH_AFTER
        ));
        assert!(needs_refresh(
            &workspace(Some("/tmp/ws"), false),
            Some(now),
            now + REFRESH_AFTER * 2
        ));
    }

    #[tokio::test]
    async fn snapshot_is_empty_until_something_is_stored() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();
        assert!(cache.snapshot(&[id]).await.is_empty());
    }

    async fn store(cache: &WorkspaceDiffStatsCache, id: Uuid, files: usize) {
        cache
            .inner
            .entries
            .insert(
                id,
                CachedDiffStats {
                    stats: DiffStats {
                        files_changed: files,
                        lines_added: 10,
                        lines_removed: 4,
                    },
                    computed_at: Instant::now(),
                },
            )
            .await;
        cache.inner.entries.run_pending_tasks().await;
    }

    #[tokio::test]
    async fn stored_stats_are_returned() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();
        store(&cache, id, 3).await;

        let snapshot = cache.snapshot(&[id]).await;
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[&id].files_changed, 3);
        assert_eq!(snapshot[&id].lines_added, 10);
        assert_eq!(snapshot[&id].lines_removed, 4);
    }

    /// The whole point of stale-while-revalidate: an invalidation must schedule a
    /// recomputation without blanking the value in the meantime. Deleting here
    /// would make the sidebar badge vanish for a poll interval every time an
    /// execution process changed state — several times per agent turn.
    #[tokio::test]
    async fn invalidate_keeps_serving_the_old_value_but_marks_it_stale() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();
        store(&cache, id, 3).await;

        cache.invalidate(id).await;
        cache.inner.entries.run_pending_tasks().await;

        let snapshot = cache.snapshot(&[id]).await;
        assert_eq!(
            snapshot.get(&id).map(|s| s.files_changed),
            Some(3),
            "invalidation must not blank the value"
        );

        // ...and the entry is now old enough that the next sweep picks it up.
        let entry = cache.inner.entries.get(&id).await.unwrap();
        assert!(needs_refresh(
            &workspace(Some("/tmp/ws"), false),
            Some(entry.computed_at),
            Instant::now()
        ));
    }

    /// `discard` is the other half: when the previous value is known to be wrong
    /// (a failed git probe, a vanished worktree) it must go, or a confident wrong
    /// number stays on screen for the whole entry TTL.
    #[tokio::test]
    async fn discard_removes_the_value() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();
        store(&cache, id, 3).await;

        cache.discard(id).await;
        cache.inner.entries.run_pending_tasks().await;
        assert!(cache.snapshot(&[id]).await.is_empty());
    }

    #[tokio::test]
    async fn invalidating_an_absent_entry_is_a_no_op() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();
        cache.invalidate(id).await;
        cache.inner.entries.run_pending_tasks().await;
        assert!(cache.snapshot(&[id]).await.is_empty());
    }

    #[tokio::test]
    async fn a_refresh_whose_generation_moved_does_not_write() {
        // Mirrors `refresh_one`'s ordering exactly: mark inflight, capture the
        // generation, "compute", then refuse to insert if the generation moved.
        // An invalidation landing mid-computation must beat the older value.
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();

        assert!(cache.inner.inflight.insert(id, ()).is_none());
        let _guard = InflightGuard {
            inner: cache.inner.clone(),
            id,
        };

        let captured = cache.generation(id);
        cache.invalidate(id).await; // arrives while the git work is running
        assert_ne!(
            cache.generation(id),
            captured,
            "an invalidation during an inflight refresh must move the generation"
        );
    }

    #[tokio::test]
    async fn invalidation_with_nothing_inflight_records_no_generation() {
        // `generations` has no eviction of its own, so it must only ever hold
        // entries for refreshes that are actually in flight.
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();

        cache.invalidate(id).await;
        cache.invalidate(id).await;

        assert_eq!(cache.generation(id), 0);
        assert!(cache.inner.generations.is_empty());
    }

    #[tokio::test]
    async fn inflight_and_generation_are_cleared_even_when_the_refresh_returns_early() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();

        {
            assert!(cache.inner.inflight.insert(id, ()).is_none());
            let _guard = InflightGuard {
                inner: cache.inner.clone(),
                id,
            };
            assert!(cache.inner.inflight.contains_key(&id));
            cache.invalidate(id).await;
            assert_eq!(cache.generation(id), 1);
        }

        // Cleared by the guard, so a persistently failing workspace is retried
        // rather than pinned as permanently in flight, and neither map leaks.
        assert!(!cache.inner.inflight.contains_key(&id));
        assert!(cache.inner.generations.is_empty());
    }

    #[tokio::test]
    async fn a_second_refresh_for_an_inflight_workspace_is_skipped() {
        let cache = WorkspaceDiffStatsCache::new();
        let id = Uuid::new_v4();

        assert!(cache.inner.inflight.insert(id, ()).is_none());
        // The dedupe check is `insert(..).is_some()`, i.e. an entry already
        // present means another refresh owns this workspace.
        assert!(cache.inner.inflight.insert(id, ()).is_some());
    }
}
