//! The automatic sweep: probe, and repair only what is expired.
//!
//! Off unless `VK_AUTO_REAUTH_INTERVAL_SECS` is set. A healthy fleet costs
//! one probe round per interval and no browser sessions. A target that keeps
//! failing waits exponentially longer (up to [`MAX_BACKOFF`]) so a broken
//! credential spends a bounded number of password/TOTP attempts per day.

use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use super::{ReauthTrigger, start_expired, wait};

/// Floor on the configured interval.
pub const MIN_INTERVAL: Duration = Duration::from_secs(300);
/// Ceiling on a failing target's backoff.
pub const MAX_BACKOFF: Duration = Duration::from_secs(6 * 60 * 60);
/// Consecutive automatic failures before the operator is paged.
pub const ESCALATE_AFTER: u32 = 3;
/// Let the server settle (and the first manual checks run) before sweeping.
const INITIAL_DELAY: Duration = Duration::from_secs(120);
/// Longest a sweep waits on one target before moving on; the run itself
/// continues and is picked up by the registry.
const PER_TARGET_WAIT: Duration = Duration::from_secs(6 * 60);

pub fn sweep_interval_from_env() -> Option<Duration> {
    interval_from(
        std::env::var("VK_AUTO_REAUTH_INTERVAL_SECS")
            .ok()
            .as_deref(),
    )
}

fn interval_from(value: Option<&str>) -> Option<Duration> {
    let secs: u64 = value?.trim().parse().ok()?;
    (secs > 0).then(|| Duration::from_secs(secs).max(MIN_INTERVAL))
}

/// When a target that has failed `failures` automatic attempts may be tried
/// again: `interval × 2^(failures−1)`, capped at [`MAX_BACKOFF`].
pub fn next_eligible(now: Instant, failures: u32, interval: Duration) -> Instant {
    let exponent = failures.saturating_sub(1).min(16);
    let backoff = interval
        .checked_mul(1u32 << exponent)
        .unwrap_or(MAX_BACKOFF)
        .min(MAX_BACKOFF);
    now + backoff
}

/// Escalate once per breakage: on a refusal, or on the third consecutive
/// automatic failure.
pub fn should_escalate(refused: bool, failures: u32, already_escalated: bool) -> bool {
    !already_escalated && (refused || failures >= ESCALATE_AFTER)
}

/// Start the sweep loop if configured. A no-op otherwise.
pub fn spawn_sweep(shutdown: CancellationToken) {
    let Some(interval) = sweep_interval_from_env() else {
        tracing::info!("unattended re-auth sweep is off (VK_AUTO_REAUTH_INTERVAL_SECS unset)");
        return;
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "unattended re-auth sweep enabled"
    );
    tokio::spawn(async move {
        tokio::select! {
            _ = shutdown.cancelled() => return,
            _ = tokio::time::sleep(INITIAL_DELAY) => {}
        }
        loop {
            // Sequential: every browser engine shares one profile anyway,
            // and one-at-a-time keeps a bad tick from fanning out.
            let started = start_expired(ReauthTrigger::Sweep).await;
            for report in started {
                tracing::info!(target = %report.id, "unattended re-auth sweep repairing");
                let _ = wait(vec![report], PER_TARGET_WAIT).await;
            }
            tokio::select! {
                _ = shutdown.cancelled() => return,
                _ = tokio::time::sleep(interval) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_is_opt_in_and_floored() {
        assert_eq!(interval_from(None), None);
        assert_eq!(interval_from(Some("0")), None);
        assert_eq!(interval_from(Some("nope")), None);
        assert_eq!(interval_from(Some("60")), Some(MIN_INTERVAL));
        assert_eq!(
            interval_from(Some(" 1800 ")),
            Some(Duration::from_secs(1800))
        );
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let now = Instant::now();
        let i = Duration::from_secs(1800);
        assert_eq!(next_eligible(now, 1, i) - now, i);
        assert_eq!(next_eligible(now, 2, i) - now, i * 2);
        assert_eq!(next_eligible(now, 3, i) - now, i * 4);
        assert_eq!(next_eligible(now, 5, i) - now, MAX_BACKOFF);
        assert_eq!(next_eligible(now, 400, i) - now, MAX_BACKOFF);
    }

    #[test]
    fn escalation_fires_once_per_breakage() {
        assert!(should_escalate(true, 0, false));
        assert!(!should_escalate(false, 2, false));
        assert!(should_escalate(false, 3, false));
        assert!(!should_escalate(false, 4, true));
        assert!(!should_escalate(true, 0, true));
    }
}
