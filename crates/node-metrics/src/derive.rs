//! Rate derivation against the previous **raw** counters.
//!
//! Every function here takes the predecessor as an `Option` and returns `None`
//! when there is nothing to derive against. That is the whole contract: a first
//! sample has no rates, and reporting `0` instead would be a fabricated
//! measurement — the thing constitution XIX prohibits outright.
//!
//! Counters are carried forward raw rather than as percentages so that a
//! delayed or skipped tick scales correctly: a 10-second gap divides by 10
//! seconds, not by the nominal 2.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::{
    parse::{NetCounters, ProcStat},
    types::{CoreBusy, NetworkSample},
};

/// A process's identity. PIDs are reused within minutes on a busy host, so
/// keying a CPU delta on the PID alone eventually attributes one process's
/// ticks to an unrelated one — as a large negative delta, or as a spike.
pub type ProcessKey = (i32, u64);

/// The raw counters carried from one sample to the next.
///
/// Deliberately *not* a [`crate::types::HostSample`]: the sample is a rendered
/// view with rates already derived, and deriving the next rate from it would
/// compound rounding and lose the counters a long gap needs.
#[derive(Debug, Clone, Default)]
pub struct Counters {
    pub captured_at: Option<DateTime<Utc>>,
    pub cpu: Option<ProcStat>,
    /// Keyed by interface name.
    pub networks: BTreeMap<String, NetCounters>,
    /// Busy ticks keyed by `(pid, start_ticks)`.
    pub processes: BTreeMap<ProcessKey, u64>,
}

/// Derived CPU busy percentages. Both fields are absent until a predecessor
/// exists.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuBusy {
    pub total_percent: Option<f32>,
    /// One entry per online core, tagged with its kernel index.
    pub per_core: Option<Vec<CoreBusy>>,
}

/// Elapsed milliseconds between two captures, or `None` if that is not a usable
/// divisor.
///
/// Zero is excluded along with absent and negative: two samples that claim the
/// same instant produce a division by zero, and a clock step backwards produces
/// a negative interval. Neither is a measurement.
pub fn elapsed_ms(previous: Option<DateTime<Utc>>, current: DateTime<Utc>) -> Option<u64> {
    let previous = previous?;
    let millis = current.signed_duration_since(previous).num_milliseconds();
    (millis > 0).then_some(millis as u64)
}

/// Busy percentage from two `/proc/stat` reads: `1 − Δidle/Δtotal`.
///
/// Returns `None` when there is no predecessor, when the counters did not move,
/// or when they moved backwards (a suspend/resume or a counter reset). The
/// per-core vector is all-or-nothing: it is replaced wholesale by consumers, so
/// a half-derived vector would silently misalign core indices.
///
/// Cores are paired by their **kernel index**, not by position: a CPU going
/// offline while another comes online keeps the line count identical while
/// shifting every position, and zipping those would attribute one core's delta
/// to another.
pub fn derive_cpu_busy(previous: Option<&ProcStat>, current: &ProcStat) -> CpuBusy {
    let Some(previous) = previous else {
        return CpuBusy::default();
    };

    let same_cores = previous.per_core.len() == current.per_core.len()
        && previous
            .per_core
            .iter()
            .zip(current.per_core.iter())
            .all(|(before, after)| before.index == after.index);

    let per_core = (!current.per_core.is_empty() && same_cores)
        .then(|| {
            previous
                .per_core
                .iter()
                .zip(current.per_core.iter())
                .map(|(before, after)| {
                    busy_percent(before.times.total, before.times.idle, &after.times).map(
                        |busy_percent| CoreBusy {
                            core: after.index,
                            busy_percent,
                        },
                    )
                })
                .collect::<Option<Vec<CoreBusy>>>()
        })
        .flatten();

    CpuBusy {
        total_percent: busy_percent(previous.total.total, previous.total.idle, &current.total),
        per_core,
    }
}

fn busy_percent(
    previous_total: u64,
    previous_idle: u64,
    current: &crate::parse::CpuTimes,
) -> Option<f32> {
    let total_delta = current.total.checked_sub(previous_total)?;
    let idle_delta = current.idle.checked_sub(previous_idle)?;
    if total_delta == 0 || idle_delta > total_delta {
        return None;
    }
    let busy = (total_delta - idle_delta) as f32 / total_delta as f32 * 100.0;
    Some(clamp_percent(busy))
}

/// Derive per-interface throughput, appending a note for any counter that went
/// backwards.
///
/// A reset interface reports `None`, not `0`. The distinction matters: `0`
/// renders as "no traffic", which is a claim about the network rather than
/// about our ability to measure it.
pub fn derive_networks(
    previous: &BTreeMap<String, NetCounters>,
    current: &[NetCounters],
    elapsed_ms: Option<u64>,
    degraded: &mut Vec<String>,
) -> Vec<NetworkSample> {
    current
        .iter()
        .map(|counters| {
            let before = previous.get(&counters.interface);
            let rx = rate_per_second(before.map(|b| b.rx_bytes), counters.rx_bytes, elapsed_ms);
            let tx = rate_per_second(before.map(|b| b.tx_bytes), counters.tx_bytes, elapsed_ms);
            if let Some(before) = before
                && (counters.rx_bytes < before.rx_bytes || counters.tx_bytes < before.tx_bytes)
            {
                degraded.push(format!(
                    "network counters for {} went backwards; throughput unavailable this sample",
                    counters.interface
                ));
            }
            NetworkSample {
                interface: counters.interface.clone(),
                rx_bytes_total: counters.rx_bytes,
                tx_bytes_total: counters.tx_bytes,
                rx_bytes_per_second: rx,
                tx_bytes_per_second: tx,
            }
        })
        .collect()
}

/// `Δvalue ÷ Δseconds`, or `None` if any input is missing or the counter moved
/// backwards. Never wraps: a wrapped `u64` reads as a multi-exabyte spike.
fn rate_per_second(previous: Option<u64>, current: u64, elapsed_ms: Option<u64>) -> Option<u64> {
    let delta = current.checked_sub(previous?)?;
    let elapsed_ms = elapsed_ms?;
    Some(delta.saturating_mul(1000) / elapsed_ms)
}

/// A process's CPU percentage: `Δ(utime+stime) / (ticks_per_second × Δs) × 100`.
///
/// `None` for a process first seen this sample. Attributing its whole lifetime
/// of ticks to one interval — which is what "no predecessor means zero delta
/// from zero" would do — puts a freshly started `cargo` at several thousand
/// percent.
///
/// Capped at `core_count × 100`: a threaded process legitimately exceeds 100%,
/// but not the machine.
///
/// A zero `elapsed_ms` or `ticks_per_second` is `None`, not a number. Both
/// would divide by zero, and `f64::min` returns the *non*-NaN operand — so
/// without this guard a process that used no CPU at all would be reported at
/// the ceiling, 600% on a six-core box.
pub fn derive_process_cpu(
    previous_ticks: Option<u64>,
    current_ticks: u64,
    elapsed_ms: Option<u64>,
    ticks_per_second: u64,
    core_count: Option<u32>,
) -> Option<f32> {
    let delta = current_ticks.checked_sub(previous_ticks?)?;
    let elapsed_ms = elapsed_ms?;
    if ticks_per_second == 0 || elapsed_ms == 0 {
        return None;
    }
    let seconds = elapsed_ms as f64 / 1000.0;
    let percent = delta as f64 / (ticks_per_second as f64 * seconds) * 100.0;
    let ceiling = core_count.map_or(f64::INFINITY, |cores| f64::from(cores) * 100.0);
    Some(clamp_percent(percent.min(ceiling) as f32))
}

/// Keep a derived percentage inside `[0, ceiling]` without inventing one.
///
/// This only trims float noise — `100.00001` from integer counters that moved
/// in lockstep. Anything that would need real clamping has already returned
/// `None` upstream.
fn clamp_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse_net_dev, parse_proc_stat};

    const STAT: &str = include_str!("../tests/fixtures/stat");
    const STAT_NEXT: &str = include_str!("../tests/fixtures/stat_next");
    const NET_DEV: &str = include_str!("../tests/fixtures/net_dev");
    const NET_DEV_NEXT: &str = include_str!("../tests/fixtures/net_dev_next");

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_800_000_000 + seconds, 0).unwrap()
    }

    fn by_interface(counters: Vec<NetCounters>) -> BTreeMap<String, NetCounters> {
        counters
            .into_iter()
            .map(|entry| (entry.interface.clone(), entry))
            .collect()
    }

    /// FR-7, stated as bluntly as it can be: with nothing to derive against,
    /// every rate is absent. A `0` here is a fabricated reading.
    #[test]
    fn a_first_sample_has_no_rates_at_all() {
        let current = parse_proc_stat(STAT).unwrap();
        let busy = derive_cpu_busy(None, &current);
        assert_eq!(busy.total_percent, None);
        assert_eq!(busy.per_core, None);

        let mut degraded = Vec::new();
        let networks = derive_networks(
            &BTreeMap::new(),
            &parse_net_dev(NET_DEV),
            None,
            &mut degraded,
        );
        assert!(!networks.is_empty());
        for interface in &networks {
            assert_eq!(interface.rx_bytes_per_second, None, "{interface:?}");
            assert_eq!(interface.tx_bytes_per_second, None, "{interface:?}");
            // The lifetime totals are real readings and are always present.
            assert!(interface.rx_bytes_total > 0 || interface.tx_bytes_total > 0);
        }
        assert!(degraded.is_empty(), "a first sample is not degraded");

        assert_eq!(
            derive_process_cpu(None, 5_000, Some(2_000), 100, Some(6)),
            None
        );
        assert_eq!(elapsed_ms(None, at(0)), None);
    }

    #[test]
    fn cpu_busy_derives_from_two_consecutive_reads() {
        let previous = parse_proc_stat(STAT).unwrap();
        let current = parse_proc_stat(STAT_NEXT).unwrap();
        let busy = derive_cpu_busy(Some(&previous), &current);

        let total = busy.total_percent.expect("total busy");
        assert!((0.0..=100.0).contains(&total), "implausible {total}");
        let per_core = busy.per_core.expect("per core busy");
        assert_eq!(per_core.len(), 6);
        for core in per_core {
            assert!(
                (0.0..=100.0).contains(&core.busy_percent),
                "implausible {core:?}"
            );
        }
    }

    #[test]
    fn cpu_busy_is_exact_for_known_counters() {
        let previous = parse_proc_stat("cpu  0 0 0 100 0\ncpu0 0 0 0 100 0\n").unwrap();
        // 100 more ticks, 25 of them idle → 75% busy.
        let current = parse_proc_stat("cpu  75 0 0 125 0\ncpu0 75 0 0 125 0\n").unwrap();
        let busy = derive_cpu_busy(Some(&previous), &current);
        assert_eq!(busy.total_percent, Some(75.0));
        assert_eq!(
            busy.per_core,
            Some(vec![CoreBusy {
                core: 0,
                busy_percent: 75.0
            }])
        );
    }

    /// The kernel index travels with the reading, so a host whose cpu1 is
    /// offline reports cpu2's utilisation *as* cpu2 rather than as whatever
    /// happens to sit at index 1.
    #[test]
    fn per_core_readings_carry_the_kernel_core_index() {
        let previous =
            parse_proc_stat("cpu  0 0 0 200 0\ncpu0 0 0 0 100 0\ncpu2 0 0 0 100 0\n").unwrap();
        let current =
            parse_proc_stat("cpu  100 0 0 300 0\ncpu0 25 0 0 175 0\ncpu2 75 0 0 125 0\n").unwrap();

        let per_core = derive_cpu_busy(Some(&previous), &current)
            .per_core
            .expect("per core busy");
        assert_eq!(
            per_core,
            vec![
                CoreBusy {
                    core: 0,
                    busy_percent: 25.0
                },
                CoreBusy {
                    core: 2,
                    busy_percent: 75.0
                },
            ]
        );
    }

    /// One core going offline while another comes online keeps the *count*
    /// identical and shifts every position. Pairing by position would charge
    /// cpu1's delta to cpu3.
    #[test]
    fn per_core_is_dropped_when_the_core_set_changes_without_changing_size() {
        let previous =
            parse_proc_stat("cpu  0 0 0 200 0\ncpu0 0 0 0 100 0\ncpu1 0 0 0 100 0\n").unwrap();
        let current =
            parse_proc_stat("cpu  100 0 0 300 0\ncpu0 25 0 0 175 0\ncpu3 75 0 0 125 0\n").unwrap();

        let busy = derive_cpu_busy(Some(&previous), &current);
        // Δtotal = 400 − 200 = 200, Δidle = 300 − 200 = 100: half the elapsed
        // jiffies were busy. The aggregate line is unaffected by the core set
        // changing underneath it.
        assert_eq!(busy.total_percent, Some(50.0));
        // cpu1 went offline and cpu3 came online, so the vector is the same
        // length but describes a different set of cores. Deriving cpu3 against
        // cpu1's counters would invent a reading, so the whole vector drops.
        assert_eq!(busy.per_core, None);
    }

    /// A suspend/resume or a hot-unplugged core makes the counters move
    /// backwards. That is not 0% busy and it is not 100% busy.
    #[test]
    fn cpu_counters_going_backwards_yield_absent_not_zero() {
        let previous = parse_proc_stat("cpu  500 0 0 500 0\n").unwrap();
        let current = parse_proc_stat("cpu  10 0 0 10 0\n").unwrap();
        assert_eq!(
            derive_cpu_busy(Some(&previous), &current).total_percent,
            None
        );

        // Idle advancing faster than the whole line is arithmetically
        // impossible and means the file was read mid-update.
        let previous = parse_proc_stat("cpu  50 0 0 100 0\n").unwrap();
        let current = parse_proc_stat("cpu  0 0 0 200 0\n").unwrap();
        let busy = derive_cpu_busy(Some(&previous), &current);
        assert_eq!(busy.total_percent, None);
    }

    #[test]
    fn cpu_busy_is_absent_when_the_counters_did_not_move() {
        let stat = parse_proc_stat(STAT).unwrap();
        let busy = derive_cpu_busy(Some(&stat), &stat);
        assert_eq!(busy.total_percent, None);
        assert_eq!(busy.per_core, None);
    }

    /// The vector is replaced wholesale by consumers, so a changed core count
    /// must drop it entirely rather than zip the shorter of the two and
    /// misalign every index.
    #[test]
    fn per_core_is_dropped_when_the_core_count_changes() {
        let previous = parse_proc_stat("cpu  0 0 0 100 0\ncpu0 0 0 0 100 0\n").unwrap();
        let current =
            parse_proc_stat("cpu  75 0 0 125 0\ncpu0 40 0 0 60 0\ncpu1 35 0 0 65 0\n").unwrap();
        let busy = derive_cpu_busy(Some(&previous), &current);
        assert_eq!(busy.total_percent, Some(75.0));
        assert_eq!(busy.per_core, None);
    }

    #[test]
    fn network_rates_derive_from_two_consecutive_reads() {
        let previous = by_interface(parse_net_dev(NET_DEV));
        let current = parse_net_dev(NET_DEV_NEXT);
        let mut degraded = Vec::new();
        // The fixtures were captured two seconds apart.
        let samples = derive_networks(&previous, &current, Some(2_000), &mut degraded);

        assert!(degraded.is_empty(), "{degraded:?}");
        let moved = samples.iter().any(|sample| {
            sample.rx_bytes_per_second.unwrap_or(0) > 0
                || sample.tx_bytes_per_second.unwrap_or(0) > 0
        });
        assert!(moved, "no interface moved between the two fixtures");
        for sample in &samples {
            assert!(sample.rx_bytes_per_second.is_some(), "{sample:?}");
            assert!(sample.tx_bytes_per_second.is_some(), "{sample:?}");
        }
    }

    #[test]
    fn network_rate_is_exact_for_known_counters() {
        let previous = by_interface(vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 1_000,
            tx_bytes: 2_000,
        }]);
        let current = vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 3_000,
            tx_bytes: 2_500,
        }];
        let mut degraded = Vec::new();
        let samples = derive_networks(&previous, &current, Some(2_000), &mut degraded);
        assert_eq!(samples[0].rx_bytes_per_second, Some(1_000));
        assert_eq!(samples[0].tx_bytes_per_second, Some(250));
        assert!(degraded.is_empty());
    }

    /// Raw counters are carried forward precisely so a long gap divides by the
    /// real elapsed time. Deriving from a stored percentage could not do this.
    #[test]
    fn a_long_gap_scales_the_rate_correctly() {
        let previous = by_interface(vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 0,
            tx_bytes: 0,
        }]);
        let current = vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 60_000,
            tx_bytes: 0,
        }];
        let mut degraded = Vec::new();

        let two_seconds = derive_networks(&previous, &current, Some(2_000), &mut degraded);
        assert_eq!(two_seconds[0].rx_bytes_per_second, Some(30_000));

        // Same counters, a minute later: 1 000 B/s, not 30 000.
        let sixty_seconds = derive_networks(&previous, &current, Some(60_000), &mut degraded);
        assert_eq!(sixty_seconds[0].rx_bytes_per_second, Some(1_000));
    }

    /// An interface reset. Not zero, not negative, and not a wrapped spike.
    #[test]
    fn a_network_counter_reset_yields_absent_and_a_degraded_note() {
        let previous = by_interface(vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 5_000_000,
            tx_bytes: 5_000_000,
        }]);
        let current = vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 1_024,
            tx_bytes: 2_048,
        }];
        let mut degraded = Vec::new();
        let samples = derive_networks(&previous, &current, Some(2_000), &mut degraded);

        assert_eq!(samples[0].rx_bytes_per_second, None);
        assert_eq!(samples[0].tx_bytes_per_second, None);
        // The post-reset lifetime totals are still real readings.
        assert_eq!(samples[0].rx_bytes_total, 1_024);
        assert_eq!(degraded.len(), 1);
        assert!(degraded[0].contains("eth0"), "{degraded:?}");
    }

    #[test]
    fn an_interface_appearing_mid_stream_has_no_rate_yet() {
        let previous = by_interface(vec![NetCounters {
            interface: "eth0".to_string(),
            rx_bytes: 10,
            tx_bytes: 10,
        }]);
        let current = vec![
            NetCounters {
                interface: "eth0".to_string(),
                rx_bytes: 20,
                tx_bytes: 20,
            },
            NetCounters {
                interface: "wg0".to_string(),
                rx_bytes: 999,
                tx_bytes: 999,
            },
        ];
        let mut degraded = Vec::new();
        let samples = derive_networks(&previous, &current, Some(1_000), &mut degraded);
        let wireguard = samples.iter().find(|s| s.interface == "wg0").unwrap();
        assert_eq!(wireguard.rx_bytes_per_second, None);
        assert!(degraded.is_empty(), "a new interface is not a reset");
    }

    #[test]
    fn process_cpu_derives_from_a_tick_delta() {
        // 100 ticks at 100 Hz over 2s on a 6-core box: one full core busy.
        assert_eq!(
            derive_process_cpu(Some(1_000), 1_200, Some(2_000), 100, Some(6)),
            Some(100.0)
        );
        // Half a core.
        assert_eq!(
            derive_process_cpu(Some(1_000), 1_100, Some(2_000), 100, Some(6)),
            Some(50.0)
        );
        // A process that used no CPU really did use none; that is a reading,
        // not an absence.
        assert_eq!(
            derive_process_cpu(Some(1_000), 1_000, Some(2_000), 100, Some(6)),
            Some(0.0)
        );
    }

    #[test]
    fn process_cpu_is_capped_at_the_machine() {
        // Absurd delta — a clock step, or a PID reused under a new identity
        // that slipped through. Capped rather than reported as 40 000%.
        let capped = derive_process_cpu(Some(0), 80_000, Some(1_000), 100, Some(6)).unwrap();
        assert_eq!(capped, 600.0);
    }

    #[test]
    fn process_cpu_counter_going_backwards_yields_absent() {
        assert_eq!(
            derive_process_cpu(Some(5_000), 10, Some(2_000), 100, Some(6)),
            None
        );
        assert_eq!(derive_process_cpu(Some(0), 100, None, 100, Some(6)), None);
        assert_eq!(
            derive_process_cpu(Some(0), 100, Some(2_000), 0, Some(6)),
            None
        );
    }

    #[test]
    fn elapsed_requires_a_forward_moving_clock() {
        assert_eq!(elapsed_ms(Some(at(0)), at(2)), Some(2_000));
        // Identical timestamps would divide by zero.
        assert_eq!(elapsed_ms(Some(at(5)), at(5)), None);
        // A clock stepped backwards is not a negative interval.
        assert_eq!(elapsed_ms(Some(at(10)), at(5)), None);
    }

    /// The delta map is keyed on `(pid, start_ticks)`, so a reused PID is a
    /// different process and starts over with no rate — rather than deriving
    /// against a stranger's tick counter.
    #[test]
    fn a_reused_pid_is_a_different_process() {
        let mut previous: BTreeMap<ProcessKey, u64> = BTreeMap::new();
        previous.insert((4242, 100), 900_000);

        let reused: ProcessKey = (4242, 5_000_000);
        assert_eq!(previous.get(&reused), None);
        assert_eq!(
            derive_process_cpu(
                previous.get(&reused).copied(),
                12,
                Some(2_000),
                100,
                Some(6)
            ),
            None
        );

        let same: ProcessKey = (4242, 100);
        assert_eq!(
            derive_process_cpu(
                previous.get(&same).copied(),
                900_200,
                Some(2_000),
                100,
                Some(6)
            ),
            Some(100.0)
        );
    }
}
