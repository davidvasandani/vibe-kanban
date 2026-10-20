//! Stop a thrashing node from admitting more work.
//!
//! On think3 (no swap) a runaway build pushed the node to ~78% iowait, and the
//! next agent it admitted blocked in D-state before writing a byte. A node in
//! that state should refuse new executions instead of wedging them. The
//! signal is PSI `full avg60` for memory and IO: `full` means every non-idle
//! task was stalled, which is thrash rather than ordinary contention, and the
//! 60 s average keeps one burst from flapping admission. Load average is not
//! used because it also counts NFS waits (wiki `coordinator-nfs-load`).

use std::{fmt, sync::Arc};

use node_metrics::parse::parse_pressure;
use serde::Serialize;

pub const WORKER_ADMISSION_MAX_PRESSURE_ENV: &str = "VK_WORKER_ADMISSION_MAX_PRESSURE";
pub const DEFAULT_ADMISSION_MAX_PRESSURE: u8 = 40;

/// Node-wide PSI `full avg60` readings, in percent. `None` where the kernel
/// has no PSI (or the platform has no `/proc`), which never refuses admission.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct PressureReading {
    pub memory_full_avg60: Option<f64>,
    pub io_full_avg60: Option<f64>,
}

impl PressureReading {
    pub fn from_host() -> Self {
        let read = |name: &str| {
            std::fs::read_to_string(format!("/proc/pressure/{name}"))
                .ok()
                .as_deref()
                .and_then(parse_pressure)
                .and_then(|pressure| pressure.full_avg60)
                .map(f64::from)
        };
        Self {
            memory_full_avg60: read("memory"),
            io_full_avg60: read("io"),
        }
    }
}

/// Why a dispatch was refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressureRefusal {
    pub resource: &'static str,
    pub full_avg60: f64,
    pub threshold: f64,
}

impl fmt::Display for PressureRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "worker is under {} pressure (full avg60 {:.1}% >= {:.0}%); not admitting new executions. Wait for pressure to fall below the threshold, then retry.",
            self.resource, self.full_avg60, self.threshold
        )
    }
}

/// Reported on `/health`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct PressureStatus {
    pub memory_full_avg60: Option<f64>,
    pub io_full_avg60: Option<f64>,
    /// `None` when admission control is disabled.
    pub threshold: Option<f64>,
    pub admission_refusing: bool,
}

type Reader = Arc<dyn Fn() -> PressureReading + Send + Sync>;

/// The admission threshold plus the source of readings (the host, or a fixed
/// value in tests).
#[derive(Clone)]
pub struct AdmissionPressure {
    /// `None` disables refusal; readings are still reported.
    threshold: Option<f64>,
    reader: Reader,
}

impl fmt::Debug for AdmissionPressure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmissionPressure")
            .field("threshold", &self.threshold)
            .finish_non_exhaustive()
    }
}

impl Default for AdmissionPressure {
    fn default() -> Self {
        Self::from_host(Some(DEFAULT_ADMISSION_MAX_PRESSURE))
    }
}

impl AdmissionPressure {
    /// `threshold` of `None` (configured as `0`) disables refusal.
    pub fn from_host(threshold: Option<u8>) -> Self {
        Self::with_reader(threshold.map(f64::from), PressureReading::from_host)
    }

    pub fn with_reader(
        threshold: Option<f64>,
        reader: impl Fn() -> PressureReading + Send + Sync + 'static,
    ) -> Self {
        Self {
            threshold,
            reader: Arc::new(reader),
        }
    }

    /// Parse the configured threshold: a whole percentage in `0..=100`, where
    /// `0` disables refusal.
    pub fn parse_threshold(value: &str) -> Option<Option<u8>> {
        let threshold = value
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|value| *value <= 100)?;
        Some((threshold > 0).then_some(threshold))
    }

    pub fn reading(&self) -> PressureReading {
        (self.reader)()
    }

    pub fn evaluate(&self, reading: PressureReading) -> Result<(), PressureRefusal> {
        let Some(threshold) = self.threshold else {
            return Ok(());
        };
        for (resource, value) in [
            ("memory", reading.memory_full_avg60),
            ("io", reading.io_full_avg60),
        ] {
            if let Some(full_avg60) = value
                && full_avg60 >= threshold
            {
                return Err(PressureRefusal {
                    resource,
                    full_avg60,
                    threshold,
                });
            }
        }
        Ok(())
    }

    pub fn check(&self) -> Result<(), PressureRefusal> {
        self.evaluate(self.reading())
    }

    pub fn status(&self) -> PressureStatus {
        let reading = self.reading();
        PressureStatus {
            memory_full_avg60: reading.memory_full_avg60,
            io_full_avg60: reading.io_full_avg60,
            threshold: self.threshold,
            admission_refusing: self.evaluate(reading).is_err(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(memory: Option<f64>, io: Option<f64>) -> impl Fn() -> PressureReading {
        move || PressureReading {
            memory_full_avg60: memory,
            io_full_avg60: io,
        }
    }

    #[test]
    fn parses_threshold_with_zero_disabling() {
        assert_eq!(AdmissionPressure::parse_threshold("40"), Some(Some(40)));
        assert_eq!(AdmissionPressure::parse_threshold(" 12 "), Some(Some(12)));
        assert_eq!(AdmissionPressure::parse_threshold("0"), Some(None));
        for invalid in ["", "-1", "101", "abc", "40%", "12.5"] {
            assert_eq!(
                AdmissionPressure::parse_threshold(invalid),
                None,
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn refuses_at_or_over_threshold_and_names_the_resource() {
        let pressure = AdmissionPressure::with_reader(Some(40.0), fixed(Some(2.0), Some(62.3)));
        let refusal = pressure.check().unwrap_err();
        assert_eq!(refusal.resource, "io");
        assert_eq!(
            refusal.to_string(),
            "worker is under io pressure (full avg60 62.3% >= 40%); not admitting new executions. Wait for pressure to fall below the threshold, then retry."
        );
        assert!(pressure.status().admission_refusing);

        let at = AdmissionPressure::with_reader(Some(40.0), fixed(Some(40.0), None));
        assert_eq!(at.check().unwrap_err().resource, "memory");
    }

    #[test]
    fn admits_below_threshold_when_disabled_or_without_psi() {
        let below = AdmissionPressure::with_reader(Some(40.0), fixed(Some(39.9), Some(0.0)));
        assert!(below.check().is_ok());
        let disabled = AdmissionPressure::with_reader(None, fixed(Some(99.0), Some(99.0)));
        assert!(disabled.check().is_ok());
        let status = disabled.status();
        assert_eq!(status.threshold, None);
        assert!(!status.admission_refusing);
        assert_eq!(status.memory_full_avg60, Some(99.0));
        let no_psi = AdmissionPressure::with_reader(Some(1.0), fixed(None, None));
        assert!(no_psi.check().is_ok());
    }
}
