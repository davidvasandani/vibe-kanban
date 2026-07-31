use std::{env, path::PathBuf, time::Duration};

use thiserror::Error;
use url::Url;
use uuid::Uuid;

pub const CLUSTER_ENABLED_ENV: &str = "VK_CLUSTER_ENABLED";
pub const COORDINATOR_ID_ENV: &str = "VK_CLUSTER_COORDINATOR_ID";
pub const SHARED_ROOT_ENV: &str = "VK_CLUSTER_SHARED_ROOT";
pub const WORKER_ENDPOINTS_ENV: &str = "VK_CLUSTER_WORKER_ENDPOINTS";
pub const HEARTBEAT_INTERVAL_SECONDS_ENV: &str = "VK_CLUSTER_HEARTBEAT_INTERVAL_SECONDS";
pub const LEASE_DURATION_SECONDS_ENV: &str = "VK_CLUSTER_LEASE_DURATION_SECONDS";
pub const LOAD_WEIGHT_ENV: &str = "VK_CLUSTER_LOAD_WEIGHT";
pub const ACTIVE_EXECUTION_WEIGHT_ENV: &str = "VK_CLUSTER_ACTIVE_EXECUTION_WEIGHT";

const DEFAULT_SHARED_ROOT: &str = "/srv/vibe-kanban-shared";
const DEFAULT_HEARTBEAT_INTERVAL_SECONDS: u64 = 10;
const DEFAULT_LEASE_DURATION_SECONDS: u64 = 30;
const DEFAULT_LOAD_WEIGHT: f64 = 1.0;
const DEFAULT_ACTIVE_EXECUTION_WEIGHT: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulingWeights {
    pub load: f64,
    pub active_executions: f64,
}

impl Default for SchedulingWeights {
    fn default() -> Self {
        Self {
            load: DEFAULT_LOAD_WEIGHT,
            active_executions: DEFAULT_ACTIVE_EXECUTION_WEIGHT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClusterConfig {
    pub enabled: bool,
    pub coordinator_id: Option<Uuid>,
    pub shared_root: PathBuf,
    pub worker_endpoints: Vec<Url>,
    pub heartbeat_interval: Duration,
    pub lease_duration: Duration,
    pub scheduling_weights: SchedulingWeights,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            coordinator_id: None,
            shared_root: PathBuf::from(DEFAULT_SHARED_ROOT),
            worker_endpoints: Vec::new(),
            heartbeat_interval: Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL_SECONDS),
            lease_duration: Duration::from_secs(DEFAULT_LEASE_DURATION_SECONDS),
            scheduling_weights: SchedulingWeights::default(),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ClusterConfigError {
    #[error("invalid value for {name}: {value:?} ({reason})")]
    InvalidValue {
        name: &'static str,
        value: String,
        reason: &'static str,
    },
    #[error("{COORDINATOR_ID_ENV} is required when clustering is enabled")]
    MissingCoordinatorId,
    #[error("{SHARED_ROOT_ENV} must be an absolute path")]
    SharedRootNotAbsolute,
    #[error("{LEASE_DURATION_SECONDS_ENV} must be greater than {HEARTBEAT_INTERVAL_SECONDS_ENV}")]
    LeaseNotLongerThanHeartbeat,
}

impl ClusterConfig {
    pub fn from_env() -> Result<Self, ClusterConfigError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ClusterConfigError> {
        let mut config = Self::default();

        if let Some(value) = nonempty(lookup(CLUSTER_ENABLED_ENV)) {
            config.enabled = parse_bool(CLUSTER_ENABLED_ENV, &value)?;
        }
        if let Some(value) = nonempty(lookup(COORDINATOR_ID_ENV)) {
            config.coordinator_id =
                Some(
                    value
                        .parse()
                        .map_err(|_| ClusterConfigError::InvalidValue {
                            name: COORDINATOR_ID_ENV,
                            value,
                            reason: "expected a UUID",
                        })?,
                );
        }
        if let Some(value) = nonempty(lookup(SHARED_ROOT_ENV)) {
            config.shared_root = PathBuf::from(value);
        }
        if let Some(value) = nonempty(lookup(WORKER_ENDPOINTS_ENV)) {
            config.worker_endpoints = parse_worker_endpoints(&value)?;
        }
        if let Some(value) = nonempty(lookup(HEARTBEAT_INTERVAL_SECONDS_ENV)) {
            config.heartbeat_interval =
                Duration::from_secs(parse_positive_u64(HEARTBEAT_INTERVAL_SECONDS_ENV, &value)?);
        }
        if let Some(value) = nonempty(lookup(LEASE_DURATION_SECONDS_ENV)) {
            config.lease_duration =
                Duration::from_secs(parse_positive_u64(LEASE_DURATION_SECONDS_ENV, &value)?);
        }
        if let Some(value) = nonempty(lookup(LOAD_WEIGHT_ENV)) {
            config.scheduling_weights.load = parse_weight(LOAD_WEIGHT_ENV, &value)?;
        }
        if let Some(value) = nonempty(lookup(ACTIVE_EXECUTION_WEIGHT_ENV)) {
            config.scheduling_weights.active_executions =
                parse_weight(ACTIVE_EXECUTION_WEIGHT_ENV, &value)?;
        }

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ClusterConfigError> {
        if self.enabled && self.coordinator_id.is_none() {
            return Err(ClusterConfigError::MissingCoordinatorId);
        }
        if !self.shared_root.is_absolute() {
            return Err(ClusterConfigError::SharedRootNotAbsolute);
        }
        if self.lease_duration <= self.heartbeat_interval {
            return Err(ClusterConfigError::LeaseNotLongerThanHeartbeat);
        }
        Ok(())
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, ClusterConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ClusterConfigError::InvalidValue {
            name,
            value: value.to_owned(),
            reason: "expected a boolean",
        }),
    }
}

fn parse_positive_u64(name: &'static str, value: &str) -> Result<u64, ClusterConfigError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| ClusterConfigError::InvalidValue {
            name,
            value: value.to_owned(),
            reason: "expected a positive integer",
        })
}

fn parse_weight(name: &'static str, value: &str) -> Result<f64, ClusterConfigError> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(|| ClusterConfigError::InvalidValue {
            name,
            value: value.to_owned(),
            reason: "expected a finite non-negative number",
        })
}

fn parse_worker_endpoints(value: &str) -> Result<Vec<Url>, ClusterConfigError> {
    value
        .split(',')
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty())
        .map(|endpoint| {
            let url = Url::parse(endpoint).map_err(|_| ClusterConfigError::InvalidValue {
                name: WORKER_ENDPOINTS_ENV,
                value: endpoint.to_owned(),
                reason: "expected a comma-separated list of absolute HTTP(S) URLs",
            })?;
            if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
                Ok(url)
            } else {
                Err(ClusterConfigError::InvalidValue {
                    name: WORKER_ENDPOINTS_ENV,
                    value: endpoint.to_owned(),
                    reason: "expected a comma-separated list of absolute HTTP(S) URLs",
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(values: &[(&str, &str)]) -> Result<ClusterConfig, ClusterConfigError> {
        let values: HashMap<_, _> = values
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        ClusterConfig::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn clustering_is_disabled_by_default() {
        assert_eq!(parse(&[]).unwrap(), ClusterConfig::default());
        assert_eq!(
            ClusterConfig::default().shared_root,
            PathBuf::from("/srv/vibe-kanban-shared")
        );
    }

    #[test]
    fn parses_complete_configuration() {
        let coordinator_id = Uuid::new_v4();
        let config = parse(&[
            (CLUSTER_ENABLED_ENV, "yes"),
            (COORDINATOR_ID_ENV, &coordinator_id.to_string()),
            (SHARED_ROOT_ENV, "/mnt/vibe"),
            (
                WORKER_ENDPOINTS_ENV,
                "http://think3:8081, https://think4.example:8081/",
            ),
            (HEARTBEAT_INTERVAL_SECONDS_ENV, "15"),
            (LEASE_DURATION_SECONDS_ENV, "45"),
            (LOAD_WEIGHT_ENV, "2.5"),
            (ACTIVE_EXECUTION_WEIGHT_ENV, "3"),
        ])
        .unwrap();

        assert!(config.enabled);
        assert_eq!(config.coordinator_id, Some(coordinator_id));
        assert_eq!(config.shared_root, PathBuf::from("/mnt/vibe"));
        assert_eq!(config.worker_endpoints.len(), 2);
        assert_eq!(config.heartbeat_interval, Duration::from_secs(15));
        assert_eq!(config.lease_duration, Duration::from_secs(45));
        assert_eq!(
            config.scheduling_weights,
            SchedulingWeights {
                load: 2.5,
                active_executions: 3.0,
            }
        );
    }

    #[test]
    fn enabled_cluster_requires_coordinator_id() {
        assert_eq!(
            parse(&[(CLUSTER_ENABLED_ENV, "true")]),
            Err(ClusterConfigError::MissingCoordinatorId)
        );
    }

    #[test]
    fn rejects_relative_root_and_non_http_endpoint() {
        assert_eq!(
            parse(&[(SHARED_ROOT_ENV, "relative/path")]),
            Err(ClusterConfigError::SharedRootNotAbsolute)
        );
        assert!(matches!(
            parse(&[(WORKER_ENDPOINTS_ENV, "ssh://think3")]),
            Err(ClusterConfigError::InvalidValue {
                name: WORKER_ENDPOINTS_ENV,
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_timing_and_weights() {
        assert_eq!(
            parse(&[
                (HEARTBEAT_INTERVAL_SECONDS_ENV, "30"),
                (LEASE_DURATION_SECONDS_ENV, "30"),
            ]),
            Err(ClusterConfigError::LeaseNotLongerThanHeartbeat)
        );
        assert!(matches!(
            parse(&[(LOAD_WEIGHT_ENV, "NaN")]),
            Err(ClusterConfigError::InvalidValue {
                name: LOAD_WEIGHT_ENV,
                ..
            })
        ));
    }
}
