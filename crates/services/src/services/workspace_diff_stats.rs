//! Shared, single-flight cache for the per-workspace diff stats shown in bulk
//! workspace summaries.
//!
//! Computing one workspace's stats runs four git subprocesses plus blob reads
//! against the worktree, and on the clustered deployment every worktree lives
//! on NFS where each `lstat` of `git status` is a round trip. The summaries
//! endpoint used to recompute every workspace on every poll of every client,
//! which kept the coordinator saturated with NFS waits. This cache makes that
//! work shared and bounded:
//!
//! - **Single-flight per workspace.** Concurrent callers for one workspace
//!   queue on its slot and reuse the leader's result.
//! - **Explicit staleness.** Each caller passes the maximum age it accepts
//!   ([`DiffStatsFreshness`]); within it no git work runs.
//! - **Process-wide bound.** At most [`BULK_DIFF_STATS_CONCURRENCY`]
//!   computations run at once, regardless of how many requests are open.
//! - **Invalidation wins races.** [`DiffStatsCache::invalidate`] bumps a
//!   generation; a computation that started before the bump never publishes.
//! - **Failures are not cached.** A `None` result is returned but not stored.

use std::{
    future::Future,
    sync::{
        Arc, LazyLock, Mutex as StdMutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use tokio::{
    sync::{Mutex, Semaphore},
    time::Instant,
};
use uuid::Uuid;

use super::diff_stream::DiffStats;

/// Process-wide cap on concurrent bulk diff-stat computations.
pub const BULK_DIFF_STATS_CONCURRENCY: usize = 4;

/// Slots whose entry is older than this (the longest tier) and that nobody is
/// using are dropped, so retained state tracks recently requested workspaces.
const PRUNE_AFTER: Duration = Duration::from_secs(60 * 60);
const PRUNE_EVERY: Duration = Duration::from_secs(60);

pub static WORKSPACE_DIFF_STATS: LazyLock<DiffStatsCache> =
    LazyLock::new(|| DiffStatsCache::new(BULK_DIFF_STATS_CONCURRENCY));

/// How stale bulk diff stats may be, by workspace state.
///
/// Any execution-process exit in the workspace invalidates its entry
/// immediately, so these bounds only apply to changes made outside Vibe Kanban
/// processes (a manual edit, a UI-driven rebase or merge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatsFreshness {
    /// A process is running in the workspace; its tree may change any moment.
    Running,
    /// Active but idle.
    Idle,
    /// Archived; nothing is expected to change.
    Archived,
}

impl DiffStatsFreshness {
    pub fn for_workspace(archived: bool, latest_running: bool) -> Self {
        if archived {
            Self::Archived
        } else if latest_running {
            Self::Running
        } else {
            Self::Idle
        }
    }

    pub fn max_age(self) -> Duration {
        match self {
            Self::Running => Duration::from_secs(30),
            Self::Idle => Duration::from_secs(5 * 60),
            Self::Archived => PRUNE_AFTER,
        }
    }
}

struct Entry {
    stats: DiffStats,
    computed_at: Instant,
    generation: u64,
}

#[derive(Default)]
struct Slot {
    state: Mutex<Option<Entry>>,
    generation: AtomicU64,
}

pub struct DiffStatsCache {
    slots: DashMap<Uuid, Arc<Slot>>,
    permits: Arc<Semaphore>,
    last_prune: StdMutex<Option<Instant>>,
}

impl DiffStatsCache {
    pub fn new(concurrency: usize) -> Self {
        Self {
            slots: DashMap::new(),
            permits: Arc::new(Semaphore::new(concurrency)),
            last_prune: StdMutex::new(None),
        }
    }

    /// Return cached stats no older than `max_age`, or compute them once.
    pub async fn get_or_compute<F>(
        &self,
        workspace_id: Uuid,
        max_age: Duration,
        compute: F,
    ) -> Option<DiffStats>
    where
        F: Future<Output = Option<DiffStats>>,
    {
        self.maybe_prune();

        let slot = self.slots.entry(workspace_id).or_default().clone();
        let mut state = slot.state.lock().await;
        let generation = slot.generation.load(Ordering::Acquire);

        if let Some(entry) = state.as_ref()
            && entry.generation == generation
            && entry.computed_at.elapsed() <= max_age
        {
            tracing::trace!(%workspace_id, "diff stats cache hit");
            return Some(entry.stats.clone());
        }

        let result = {
            // The semaphore is never closed, so acquire cannot fail.
            let _permit = self.permits.acquire().await.ok()?;
            compute.await
        };

        match &result {
            Some(stats) if slot.generation.load(Ordering::Acquire) == generation => {
                *state = Some(Entry {
                    stats: stats.clone(),
                    computed_at: Instant::now(),
                    generation,
                });
                tracing::debug!(%workspace_id, "diff stats computed and cached");
            }
            Some(_) => {
                *state = None;
                tracing::debug!(
                    %workspace_id,
                    "diff stats invalidated during computation; result not cached"
                );
            }
            None => {
                tracing::debug!(%workspace_id, "diff stats computation failed; not cached");
            }
        }
        result
    }

    /// Discard the workspace's cached stats and prevent any in-flight
    /// computation from publishing its (older) result.
    pub fn invalidate(&self, workspace_id: Uuid) {
        if let Some(slot) = self.slots.get(&workspace_id) {
            slot.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn maybe_prune(&self) {
        let now = Instant::now();
        {
            let Ok(mut last) = self.last_prune.lock() else {
                return;
            };
            if last.is_some_and(|at| now.duration_since(at) < PRUNE_EVERY) {
                return;
            }
            *last = Some(now);
        }
        self.slots.retain(|_, slot| {
            // In use: a caller holds a clone (or is waiting on the lock).
            if Arc::strong_count(slot) > 1 {
                return true;
            }
            match slot.state.try_lock() {
                Ok(state) => state
                    .as_ref()
                    .is_some_and(|entry| now.duration_since(entry.computed_at) <= PRUNE_AFTER),
                Err(_) => true,
            }
        });
    }

    #[cfg(test)]
    fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    fn stats(files: usize) -> DiffStats {
        DiffStats {
            files_changed: files,
            lines_added: files * 10,
            lines_removed: files,
        }
    }

    async fn counted(counter: &AtomicUsize, files: usize) -> Option<DiffStats> {
        counter.fetch_add(1, Ordering::SeqCst);
        Some(stats(files))
    }

    #[tokio::test(start_paused = true)]
    async fn returns_cached_stats_within_max_age() {
        let cache = DiffStatsCache::new(4);
        let id = Uuid::new_v4();
        let calls = AtomicUsize::new(0);
        let max_age = Duration::from_secs(30);

        let first = cache.get_or_compute(id, max_age, counted(&calls, 1)).await;
        tokio::time::advance(Duration::from_secs(29)).await;
        let second = cache.get_or_compute(id, max_age, counted(&calls, 2)).await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(first.unwrap().files_changed, 1);
        assert_eq!(second.unwrap().files_changed, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn recomputes_after_max_age() {
        let cache = DiffStatsCache::new(4);
        let id = Uuid::new_v4();
        let calls = AtomicUsize::new(0);
        let max_age = Duration::from_secs(30);

        cache.get_or_compute(id, max_age, counted(&calls, 1)).await;
        tokio::time::advance(Duration::from_secs(31)).await;
        let fresh = cache.get_or_compute(id, max_age, counted(&calls, 2)).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(fresh.unwrap().files_changed, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_shorter_max_age_forces_recompute_of_an_entry_cached_for_longer() {
        let cache = DiffStatsCache::new(4);
        let id = Uuid::new_v4();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_compute(id, DiffStatsFreshness::Idle.max_age(), counted(&calls, 1))
            .await;
        tokio::time::advance(Duration::from_secs(45)).await;
        cache
            .get_or_compute(
                id,
                DiffStatsFreshness::Running.max_age(),
                counted(&calls, 2),
            )
            .await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_callers_share_one_computation() {
        let cache = Arc::new(DiffStatsCache::new(4));
        let id = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let cache = cache.clone();
                let calls = calls.clone();
                tokio::spawn(async move {
                    cache
                        .get_or_compute(id, Duration::from_secs(30), async {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            Some(stats(3))
                        })
                        .await
                })
            })
            .collect();

        for handle in handles {
            assert_eq!(handle.await.unwrap().unwrap().files_changed, 3);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn invalidate_forces_recompute() {
        let cache = DiffStatsCache::new(4);
        let id = Uuid::new_v4();
        let calls = AtomicUsize::new(0);
        let max_age = Duration::from_secs(300);

        cache.get_or_compute(id, max_age, counted(&calls, 1)).await;
        cache.invalidate(id);
        let fresh = cache.get_or_compute(id, max_age, counted(&calls, 2)).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(fresh.unwrap().files_changed, 2);
    }

    #[tokio::test(start_paused = true)]
    async fn invalidation_during_computation_blocks_stale_write_back() {
        let cache = Arc::new(DiffStatsCache::new(4));
        let id = Uuid::new_v4();
        let calls = Arc::new(AtomicUsize::new(0));
        let max_age = Duration::from_secs(300);

        let leader = {
            let cache = cache.clone();
            let calls = calls.clone();
            tokio::spawn(async move {
                cache
                    .get_or_compute(id, max_age, async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        Some(stats(1))
                    })
                    .await
            })
        };
        // Let the leader start computing, then invalidate mid-flight.
        tokio::time::sleep(Duration::from_secs(1)).await;
        cache.invalidate(id);

        // The leader's caller still gets its result...
        assert_eq!(leader.await.unwrap().unwrap().files_changed, 1);
        // ...but it was not stored, so the next reader recomputes.
        let next = cache.get_or_compute(id, max_age, counted(&calls, 2)).await;
        assert_eq!(next.unwrap().files_changed, 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn failures_are_not_cached() {
        let cache = DiffStatsCache::new(4);
        let id = Uuid::new_v4();
        let calls = AtomicUsize::new(0);
        let max_age = Duration::from_secs(300);

        let failed = cache
            .get_or_compute(id, max_age, async {
                calls.fetch_add(1, Ordering::SeqCst);
                None
            })
            .await;
        let retried = cache.get_or_compute(id, max_age, counted(&calls, 4)).await;

        assert!(failed.is_none());
        assert_eq!(retried.unwrap().files_changed, 4);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrency_is_bounded_across_workspaces() {
        let cache = Arc::new(DiffStatsCache::new(2));
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..6)
            .map(|_| {
                let cache = cache.clone();
                let running = running.clone();
                let peak = peak.clone();
                tokio::spawn(async move {
                    cache
                        .get_or_compute(Uuid::new_v4(), Duration::from_secs(30), async {
                            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                            peak.fetch_max(now, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            running.fetch_sub(1, Ordering::SeqCst);
                            Some(stats(1))
                        })
                        .await
                })
            })
            .collect();
        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn idle_expired_slots_are_pruned() {
        let cache = DiffStatsCache::new(4);
        let old = Uuid::new_v4();
        let calls = AtomicUsize::new(0);

        cache
            .get_or_compute(old, Duration::from_secs(30), counted(&calls, 1))
            .await;
        tokio::time::advance(PRUNE_AFTER + PRUNE_EVERY + Duration::from_secs(1)).await;
        cache
            .get_or_compute(Uuid::new_v4(), Duration::from_secs(30), counted(&calls, 1))
            .await;

        assert_eq!(cache.slot_count(), 1);
    }

    #[test]
    fn freshness_tiers_follow_workspace_state() {
        assert_eq!(
            DiffStatsFreshness::for_workspace(true, true),
            DiffStatsFreshness::Archived
        );
        assert_eq!(
            DiffStatsFreshness::for_workspace(false, true),
            DiffStatsFreshness::Running
        );
        assert_eq!(
            DiffStatsFreshness::for_workspace(false, false),
            DiffStatsFreshness::Idle
        );
        assert_eq!(
            DiffStatsFreshness::Running.max_age(),
            Duration::from_secs(30)
        );
        assert_eq!(DiffStatsFreshness::Idle.max_age(), Duration::from_secs(300));
        assert_eq!(
            DiffStatsFreshness::Archived.max_age(),
            Duration::from_secs(3600)
        );
    }
}
