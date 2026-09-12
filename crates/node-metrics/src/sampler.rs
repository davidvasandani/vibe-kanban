//! The bounded ring of samples and the ticker that fills it.
//!
//! Three properties are load-bearing, and each of them is a bug that has bitten
//! this codebase before in another guise:
//!
//! 1. **Memory does not grow with uptime.** The ring is fixed-size and the
//!    process table — roughly 80% of a sample — is kept on the newest entry
//!    only. A stream whose payload grows with elapsed time is what exhausted
//!    the server in the log-compaction incident.
//! 2. **The ticker terminates.** It holds a [`Weak`] and re-checks every tick
//!    that someone still owns the sampler. A spawned loop that keeps a strong
//!    handle keeps its owner alive forever.
//! 3. **A gap is visible.** [`MetricsSampler::since`] reports what was evicted
//!    rather than silently returning a short batch, so a consumer can resnapshot
//!    instead of drawing a straight line through missing time.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{
    collect::{CollectError, Collector},
    derive::Counters,
    types::{HostSample, SampleBatch, SamplerConfig},
};

/// Everything behind the lock. Kept in one struct so the ring, the cursor, and
/// the carried counters cannot drift out of step with each other.
#[derive(Debug, Default)]
struct Ring {
    samples: VecDeque<HostSample>,
    counters: Counters,
    /// Sequence of the most recently appended sample; 0 before the first.
    latest: u64,
}

pub struct MetricsSampler {
    collector: Collector,
    ring: Mutex<Ring>,
    /// Read without the lock so a consumer can cheaply check for new data.
    latest_sequence: AtomicU64,
}

impl std::fmt::Debug for MetricsSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsSampler")
            .field(
                "latest_sequence",
                &self.latest_sequence.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

impl MetricsSampler {
    pub fn new(config: SamplerConfig) -> Self {
        Self {
            collector: Collector::new(config),
            ring: Mutex::new(Ring::default()),
            latest_sequence: AtomicU64::new(0),
        }
    }

    pub fn config(&self) -> &SamplerConfig {
        self.collector.config()
    }

    /// Sequence of the newest retained sample, or 0 if none.
    pub fn latest_sequence(&self) -> u64 {
        self.latest_sequence.load(Ordering::Relaxed)
    }

    /// Read the host once and append the result.
    ///
    /// Blocking: callers on an async runtime must wrap this in
    /// `spawn_blocking`. [`MetricsSampler::spawn`] does.
    pub fn sample_now(&self) -> Result<(), CollectError> {
        // The previous counters are cloned out and the lock released before the
        // read, so a slow `/proc` walk — a stalled NFS `statvfs` is the
        // realistic case — never blocks a reader.
        let (previous, next_sequence) = {
            let ring = self.lock();
            (ring.counters.clone(), ring.latest + 1)
        };

        let collected = self.collector.collect(&previous, next_sequence)?;

        let mut ring = self.lock();
        // Another sample may have landed while the lock was released. Sequences
        // must stay strictly monotonic, so re-stamp rather than overwrite.
        let sequence = ring.latest + 1;
        let mut sample = collected.sample;
        sample.sequence = sequence;

        // Only the newest sample carries a process table; demote the previous
        // newest as it becomes history.
        if let Some(previous_newest) = ring.samples.back_mut() {
            previous_newest.processes = None;
        }

        ring.samples.push_back(sample);
        ring.counters = collected.counters;
        ring.latest = sequence;

        let retention = self.config().retention as usize;
        while ring.samples.len() > retention.max(1) {
            ring.samples.pop_front();
        }

        self.latest_sequence.store(sequence, Ordering::Relaxed);
        Ok(())
    }

    /// Samples with `sequence > after`, oldest first.
    ///
    /// `earliest_retained_sequence` lets the caller detect that its cursor fell
    /// out of the ring; see [`SampleBatch::has_gap`].
    pub fn since(&self, after: u64) -> SampleBatch {
        let ring = self.lock();
        let earliest_retained_sequence = ring
            .samples
            .front()
            .map(|sample| sample.sequence)
            // An empty ring has retained nothing; reporting 0 keeps `has_gap`
            // false, because nothing was missed — there was nothing to miss.
            .unwrap_or(0);

        SampleBatch {
            samples: ring
                .samples
                .iter()
                .filter(|sample| sample.sequence > after)
                .cloned()
                .collect(),
            earliest_retained_sequence,
            latest_sequence: ring.latest,
        }
    }

    /// Drive `sampler` on its configured interval until every strong reference
    /// is dropped or `shutdown` resolves.
    ///
    /// Takes an `Arc` and immediately downgrades it: the loop must not be what
    /// keeps the sampler alive. Returns once the owner is gone, which is what
    /// makes "the collector stops when nobody is looking" true by construction
    /// rather than by remembering to cancel a handle.
    pub fn spawn(
        sampler: &Arc<Self>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let weak = Arc::downgrade(sampler);
        let interval = sampler.config().interval();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // A delayed tick must not be followed by a burst of catch-up ticks;
            // each sample is a measurement of *now*, not a slot to backfill.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {}
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() {
                            return;
                        }
                        continue;
                    }
                }

                // Upgrade only for the duration of one sample, so between ticks
                // this task holds nothing.
                let Some(sampler) = Weak::upgrade(&weak) else {
                    return;
                };

                let result = tokio::task::spawn_blocking(move || sampler.sample_now()).await;

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        // Sampling failing is not fatal: `/proc` can be briefly
                        // unreadable, and the consumer already renders absence.
                        tracing::debug!(%error, "host metrics sample failed");
                    }
                    Err(error) if error.is_panic() => {
                        tracing::warn!(%error, "host metrics sampler panicked");
                    }
                    Err(_) => return,
                }
            }
        })
    }

    /// A poisoned lock means a previous holder panicked mid-update. The ring is
    /// a cache of observations, not a ledger — recovering and continuing to
    /// report is strictly better than propagating a panic through every reader.
    fn lock(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.ring
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn sampler(retention: u32) -> MetricsSampler {
        MetricsSampler::new(SamplerConfig {
            retention,
            ..SamplerConfig::default()
        })
    }

    #[test]
    fn sequences_start_at_one_and_increase() {
        let sampler = sampler(10);
        assert_eq!(sampler.latest_sequence(), 0);
        sampler.sample_now().unwrap();
        assert_eq!(sampler.latest_sequence(), 1);
        sampler.sample_now().unwrap();
        assert_eq!(sampler.latest_sequence(), 2);
    }

    #[test]
    fn ring_evicts_the_oldest_beyond_retention() {
        let sampler = sampler(3);
        for _ in 0..5 {
            sampler.sample_now().unwrap();
        }

        let batch = sampler.since(0);
        assert_eq!(batch.samples.len(), 3);
        assert_eq!(batch.earliest_retained_sequence, 3);
        assert_eq!(batch.latest_sequence, 5);
        let sequences: Vec<u64> = batch.samples.iter().map(|s| s.sequence).collect();
        assert_eq!(sequences, vec![3, 4, 5]);
    }

    #[test]
    fn since_returns_only_newer_samples() {
        let sampler = sampler(10);
        for _ in 0..4 {
            sampler.sample_now().unwrap();
        }

        let batch = sampler.since(2);
        let sequences: Vec<u64> = batch.samples.iter().map(|s| s.sequence).collect();
        assert_eq!(sequences, vec![3, 4]);
        assert!(!batch.has_gap(2));
    }

    /// The gap contract: a consumer whose cursor was evicted must be told, so it
    /// resnapshots instead of drawing a line across missing time.
    #[test]
    fn an_evicted_cursor_is_reported_as_a_gap() {
        let sampler = sampler(2);
        for _ in 0..5 {
            sampler.sample_now().unwrap();
        }

        let batch = sampler.since(1);
        assert_eq!(batch.earliest_retained_sequence, 4);
        assert!(
            batch.has_gap(1),
            "cursor 1 needs sample 2, which was evicted"
        );
        // A cold consumer asked for everything and missed nothing.
        assert!(!batch.has_gap(0));
    }

    /// The memory decision from clarification C4, asserted where it can regress:
    /// history must not carry process tables.
    #[test]
    fn only_the_newest_sample_carries_processes() {
        let sampler = sampler(10);
        for _ in 0..3 {
            sampler.sample_now().unwrap();
        }

        let batch = sampler.since(0);
        let (newest, history) = batch.samples.split_last().unwrap();
        assert!(
            newest.processes.is_some(),
            "newest sample must carry the table"
        );
        for sample in history {
            assert!(
                sample.processes.is_none(),
                "history sample {} retained a process table",
                sample.sequence
            );
        }
    }

    #[test]
    fn an_empty_ring_reports_no_gap() {
        let sampler = sampler(10);
        let batch = sampler.since(0);
        assert!(batch.samples.is_empty());
        assert_eq!(batch.latest_sequence, 0);
        assert!(!batch.has_gap(0));
    }

    /// Property 2: dropping the last strong reference must end the ticker.
    /// A loop holding an `Arc` would keep the sampler — and its owner — alive
    /// forever, which is the leak this test exists to catch.
    #[tokio::test]
    async fn ticker_stops_when_the_last_owner_is_dropped() {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let sampler = Arc::new(MetricsSampler::new(SamplerConfig {
            interval_ms: 10,
            ..SamplerConfig::default()
        }));
        let handle = MetricsSampler::spawn(&sampler, rx);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(sampler);

        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("ticker should exit once its owner is dropped")
            .expect("ticker should not panic");
    }

    #[tokio::test]
    async fn ticker_stops_on_shutdown_signal() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let sampler = Arc::new(MetricsSampler::new(SamplerConfig {
            interval_ms: 10,
            ..SamplerConfig::default()
        }));
        let handle = MetricsSampler::spawn(&sampler, rx);

        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        tx.send(true).unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(5), handle)
            .await
            .expect("ticker should exit on shutdown")
            .expect("ticker should not panic");
        // The sampler outlives its ticker; the loop held only a Weak.
        assert!(Arc::strong_count(&sampler) >= 1);
    }

    #[tokio::test]
    async fn ticker_actually_samples() {
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let sampler = Arc::new(MetricsSampler::new(SamplerConfig {
            interval_ms: 10,
            ..SamplerConfig::default()
        }));
        let handle = MetricsSampler::spawn(&sampler, rx);

        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        assert!(
            sampler.latest_sequence() >= 2,
            "expected several samples, got {}",
            sampler.latest_sequence()
        );

        handle.abort();
    }
}
