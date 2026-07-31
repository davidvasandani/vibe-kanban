use std::{net::SocketAddr, path::PathBuf, str::FromStr};

use axum::{Json, Router, routing::get};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod journal;
pub mod mount_health;
pub mod path_authority;

pub const WORKER_NODE_ID_ENV: &str = "VK_WORKER_NODE_ID";
pub const WORKER_LISTEN_ADDR_ENV: &str = "VK_WORKER_LISTEN_ADDR";
pub const WORKER_SHARED_ROOT_ENV: &str = "VK_CLUSTER_SHARED_ROOT";
pub const WORKER_COORDINATOR_URL_ENV: &str = "VK_WORKER_COORDINATOR_URL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub worker_node_id: Uuid,
    pub listen_addr: SocketAddr,
    pub shared_root: PathBuf,
    pub coordinator_url: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerConfigError {
    #[error("{0} is required")]
    Missing(&'static str),
    #[error("invalid {name}: {value:?}")]
    Invalid { name: &'static str, value: String },
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, WorkerConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, WorkerConfigError> {
        let required = |name: &'static str| {
            lookup(name)
                .filter(|value| !value.trim().is_empty())
                .ok_or(WorkerConfigError::Missing(name))
        };
        let worker_node_id = parse(WORKER_NODE_ID_ENV, required(WORKER_NODE_ID_ENV)?)?;
        let listen_addr = lookup(WORKER_LISTEN_ADDR_ENV)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "0.0.0.0:8086".into());
        let listen_addr = parse(WORKER_LISTEN_ADDR_ENV, listen_addr)?;
        let shared_root = PathBuf::from(
            lookup(WORKER_SHARED_ROOT_ENV)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "/srv/vibe-kanban-shared".into()),
        );
        if !shared_root.is_absolute() {
            return Err(WorkerConfigError::Invalid {
                name: WORKER_SHARED_ROOT_ENV,
                value: shared_root.display().to_string(),
            });
        }
        let coordinator_url = required(WORKER_COORDINATOR_URL_ENV)?;
        if !(coordinator_url.starts_with("http://") || coordinator_url.starts_with("https://")) {
            return Err(WorkerConfigError::Invalid {
                name: WORKER_COORDINATOR_URL_ENV,
                value: coordinator_url,
            });
        }
        Ok(Self {
            worker_node_id,
            listen_addr,
            shared_root,
            coordinator_url,
        })
    }
}

fn parse<T: FromStr>(name: &'static str, value: String) -> Result<T, WorkerConfigError> {
    value
        .parse()
        .map_err(|_| WorkerConfigError::Invalid { name, value })
}

#[derive(Debug, Serialize)]
struct Health {
    status: &'static str,
    worker_node_id: Uuid,
}

pub async fn run(config: WorkerConfig, shutdown: CancellationToken) -> anyhow::Result<()> {
    let worker_node_id = config.worker_node_id;
    let router = Router::new().route(
        "/health",
        get(move || async move {
            Json(Health {
                status: "ok",
                worker_node_id,
            })
        }),
    );
    let listener = TcpListener::bind(config.listen_addr).await?;
    tracing::info!(
        worker_node_id = %config.worker_node_id,
        listen_addr = %listener.local_addr()?,
        shared_root = %config.shared_root.display(),
        coordinator_url = %config.coordinator_url,
        "vibe kanban worker listening"
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn parse(values: &[(&str, &str)]) -> Result<WorkerConfig, WorkerConfigError> {
        let values: HashMap<_, _> = values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        WorkerConfig::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn parses_required_identity_and_coordinator_with_safe_defaults() {
        let id = Uuid::new_v4();
        let config = parse(&[
            (WORKER_NODE_ID_ENV, &id.to_string()),
            (WORKER_COORDINATOR_URL_ENV, "http://think2:3333"),
        ])
        .unwrap();
        assert_eq!(config.worker_node_id, id);
        assert_eq!(config.listen_addr, "0.0.0.0:8086".parse().unwrap());
        assert_eq!(config.shared_root, PathBuf::from("/srv/vibe-kanban-shared"));
    }

    #[test]
    fn rejects_missing_identity_and_relative_shared_root() {
        assert_eq!(
            parse(&[(WORKER_COORDINATOR_URL_ENV, "http://think2:3333")]),
            Err(WorkerConfigError::Missing(WORKER_NODE_ID_ENV))
        );
        let id = Uuid::new_v4();
        assert!(matches!(
            parse(&[
                (WORKER_NODE_ID_ENV, &id.to_string()),
                (WORKER_COORDINATOR_URL_ENV, "http://think2:3333"),
                (WORKER_SHARED_ROOT_ENV, "relative"),
            ]),
            Err(WorkerConfigError::Invalid {
                name: WORKER_SHARED_ROOT_ENV,
                ..
            })
        ));
    }
}
