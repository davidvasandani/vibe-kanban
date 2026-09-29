use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{Json, Router, routing::get};
use job_cgroup::{
    JobCgroupConfig, JobCgroupStatus, MemoryLimit, WORKER_JOB_CGROUP_ENV,
    WORKER_JOB_MEMORY_HIGH_ENV, WORKER_JOB_MEMORY_MAX_ENV,
};
use node_metrics::{MetricsSampler, types::SamplerConfig};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod cancellation;
pub mod execution;
pub mod interaction;
pub mod job_cgroup;
pub mod journal;
pub mod mount_health;
pub mod path_authority;
pub mod preview;
pub mod recovery;
pub mod server;
pub mod terminal;
pub mod worker_api;

pub const WORKER_NODE_ID_ENV: &str = "VK_WORKER_NODE_ID";
pub const WORKER_LISTEN_ADDR_ENV: &str = "VK_WORKER_LISTEN_ADDR";
pub const WORKER_SHARED_ROOT_ENV: &str = "VK_CLUSTER_SHARED_ROOT";
pub const WORKER_COORDINATOR_URL_ENV: &str = "VK_WORKER_COORDINATOR_URL";
pub const WORKER_COORDINATOR_ID_ENV: &str = "VK_CLUSTER_COORDINATOR_ID";
pub const WORKER_SIGNING_KEY_FILE_ENV: &str = "VK_WORKER_SIGNING_KEY_FILE";
pub const COORDINATOR_PUBLIC_KEY_FILE_ENV: &str = "VK_COORDINATOR_PUBLIC_KEY_FILE";
pub const WORKER_EXPECTED_EXPORT_ENV: &str = "VK_CLUSTER_EXPECTED_FILESYSTEM_ID";
pub const WORKER_EXPECTED_UID_ENV: &str = "VK_WORKER_EXPECTED_UID";
pub const WORKER_EXPECTED_GID_ENV: &str = "VK_WORKER_EXPECTED_GID";
pub const WORKER_EXECUTOR_PROFILES_ENV: &str = "VK_WORKER_EXECUTOR_PROFILES";
pub const WORKER_STATE_DIR_ENV: &str = "VK_WORKER_STATE_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub worker_node_id: Uuid,
    pub listen_addr: SocketAddr,
    pub shared_root: PathBuf,
    pub coordinator_url: String,
    pub coordinator_id: Uuid,
    pub signing_key_file: PathBuf,
    pub coordinator_public_key_file: PathBuf,
    pub expected_export: String,
    pub expected_uid: u32,
    pub expected_gid: u32,
    pub executor_profiles: Vec<String>,
    pub state_dir: PathBuf,
    /// Isolate spawned children from the worker's own cgroup. `None` keeps
    /// the historical single-cgroup layout (local and undelegated runs).
    pub job_cgroup: Option<JobCgroupConfig>,
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
        let worker_node_id: Uuid = parse(WORKER_NODE_ID_ENV, required(WORKER_NODE_ID_ENV)?)?;
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
        let valid_coordinator_url = reqwest::Url::parse(&coordinator_url).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        });
        if !valid_coordinator_url {
            return Err(WorkerConfigError::Invalid {
                name: WORKER_COORDINATOR_URL_ENV,
                value: coordinator_url,
            });
        }
        let coordinator_id = parse(
            WORKER_COORDINATOR_ID_ENV,
            required(WORKER_COORDINATOR_ID_ENV)?,
        )?;
        let signing_key_file = PathBuf::from(required(WORKER_SIGNING_KEY_FILE_ENV)?);
        if !signing_key_file.is_absolute() {
            return Err(WorkerConfigError::Invalid {
                name: WORKER_SIGNING_KEY_FILE_ENV,
                value: signing_key_file.display().to_string(),
            });
        }
        let coordinator_public_key_file = PathBuf::from(required(COORDINATOR_PUBLIC_KEY_FILE_ENV)?);
        if !coordinator_public_key_file.is_absolute() {
            return Err(WorkerConfigError::Invalid {
                name: COORDINATOR_PUBLIC_KEY_FILE_ENV,
                value: coordinator_public_key_file.display().to_string(),
            });
        }
        let expected_export = lookup(WORKER_EXPECTED_EXPORT_ENV)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "172.16.0.99:/var/nfs/shared/VibeKanban".into());
        let expected_uid = parse(WORKER_EXPECTED_UID_ENV, required(WORKER_EXPECTED_UID_ENV)?)?;
        let expected_gid = parse(WORKER_EXPECTED_GID_ENV, required(WORKER_EXPECTED_GID_ENV)?)?;
        let executor_profiles = lookup(WORKER_EXECUTOR_PROFILES_ENV)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|profile| !profile.is_empty())
            .map(str::to_owned)
            .collect();
        let state_dir = lookup(WORKER_STATE_DIR_ENV)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                shared_root
                    .join("execution-logs/worker-state")
                    .join(worker_node_id.to_string())
            });
        if !state_dir.is_absolute() {
            return Err(WorkerConfigError::Invalid {
                name: WORKER_STATE_DIR_ENV,
                value: state_dir.display().to_string(),
            });
        }
        let job_cgroup = parse_job_cgroup(&lookup)?;
        Ok(Self {
            worker_node_id,
            listen_addr,
            shared_root,
            coordinator_url,
            coordinator_id,
            signing_key_file,
            coordinator_public_key_file,
            expected_export,
            expected_uid,
            expected_gid,
            executor_profiles,
            state_dir,
            job_cgroup,
        })
    }
}

fn parse_job_cgroup(
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<Option<JobCgroupConfig>, WorkerConfigError> {
    let Some(name) = lookup(WORKER_JOB_CGROUP_ENV).filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    if !JobCgroupConfig::valid_name(&name) {
        return Err(WorkerConfigError::Invalid {
            name: WORKER_JOB_CGROUP_ENV,
            value: name,
        });
    }
    let limit = |env: &'static str| match lookup(env).filter(|value| !value.trim().is_empty()) {
        None => Ok(MemoryLimit::Unlimited),
        Some(value) => {
            MemoryLimit::parse(&value).ok_or(WorkerConfigError::Invalid { name: env, value })
        }
    };
    Ok(Some(JobCgroupConfig {
        name,
        memory_high: limit(WORKER_JOB_MEMORY_HIGH_ENV)?,
        memory_max: limit(WORKER_JOB_MEMORY_MAX_ENV)?,
    }))
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
    active_execution_count: u32,
    admission_draining: bool,
    drain_safe: bool,
    job_cgroup: JobCgroupStatus,
}

impl Health {
    fn new(
        worker_node_id: Uuid,
        active_execution_count: u32,
        admission_draining: bool,
        job_cgroup: JobCgroupStatus,
    ) -> Self {
        Self {
            status: "ok",
            worker_node_id,
            active_execution_count,
            admission_draining,
            drain_safe: admission_draining && active_execution_count == 0,
            job_cgroup,
        }
    }
}

pub async fn run(config: WorkerConfig, shutdown: CancellationToken) -> anyhow::Result<()> {
    run_with_drain(config, shutdown, Arc::new(AtomicBool::new(false))).await
}

async fn reset_mcp_config_root(root: &Path) -> std::io::Result<()> {
    match tokio::fs::remove_dir_all(root).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    tokio::fs::create_dir_all(root).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        tokio::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).await?;
    }
    Ok(())
}

pub async fn run_with_drain(
    config: WorkerConfig,
    shutdown: CancellationToken,
    admission_draining: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    // Before recovery or any executor can spawn a child into the supervisor's
    // cgroup; the sweeper keeps moving later children for the process lifetime.
    let job_cgroup_handle = job_cgroup::start(config.job_cgroup.as_ref(), shutdown.child_token());
    let path_authority = path_authority::PathAuthority::new(&config.shared_root)?;
    let coordinator_url = reqwest::Url::parse(&config.coordinator_url)?;
    let mcp_config_root = config.state_dir.join("mcp-config");
    // Recovery interrupts formerly active jobs instead of resuming them, so no
    // execution owns a scoped native config when this process starts.
    reset_mcp_config_root(&mcp_config_root).await?;
    let supervisor = execution::ExecutionSupervisor::with_recovery_and_drain(
        path_authority.clone(),
        recovery::RecoveryStore::new(&config.state_dir).await?,
        admission_draining.clone(),
        mcp_config_root,
        coordinator_url,
    )
    .await?;
    let coordinator_task = tokio::spawn(server::registration_loop(
        config.clone(),
        supervisor.clone(),
        shutdown.child_token(),
    ));
    // Host metrics sample continuously, independently of the registration and
    // heartbeat loop: the sampler feeds `GET /v1/metrics` only and is not part
    // of the evidence channel. The ticker holds a `Weak`, so this `Arc` and the
    // shutdown signal are jointly what keep it alive.
    let metrics = Arc::new(MetricsSampler::new(SamplerConfig::default()));
    let metrics_task = {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let shutdown = shutdown.child_token();
        tokio::spawn(async move {
            shutdown.cancelled().await;
            let _ = tx.send(true);
        });
        MetricsSampler::spawn(&metrics, rx)
    };
    let worker_node_id = config.worker_node_id;
    let health_supervisor = supervisor.clone();
    let health_admission_draining = admission_draining.clone();
    let health_job_cgroup = job_cgroup_handle.clone();
    let router = Router::new()
        .route(
            "/health",
            get(move || async move {
                let active_execution_count = health_supervisor.active_execution_count().await;
                Json(Health::new(
                    worker_node_id,
                    active_execution_count,
                    health_admission_draining.load(Ordering::Acquire),
                    health_job_cgroup.status(),
                ))
            }),
        )
        .merge(worker_api::router(&config, supervisor, metrics.clone()).await?);
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
    coordinator_task.abort();
    metrics_task.abort();
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
        let coordinator_id = Uuid::new_v4();
        let config = parse(&[
            (WORKER_NODE_ID_ENV, &id.to_string()),
            (WORKER_COORDINATOR_URL_ENV, "http://think2:3333"),
            (WORKER_COORDINATOR_ID_ENV, &coordinator_id.to_string()),
            (WORKER_SIGNING_KEY_FILE_ENV, "/run/credentials/worker.key"),
            (
                COORDINATOR_PUBLIC_KEY_FILE_ENV,
                "/run/credentials/coordinator.pub",
            ),
            (WORKER_EXPECTED_UID_ENV, "1000"),
            (WORKER_EXPECTED_GID_ENV, "100"),
        ])
        .unwrap();
        assert_eq!(config.worker_node_id, id);
        assert_eq!(config.coordinator_id, coordinator_id);
        assert_eq!(
            config.listen_addr,
            "0.0.0.0:8086".parse::<std::net::SocketAddr>().unwrap()
        );
        assert_eq!(config.shared_root, PathBuf::from("/srv/vibe-kanban-shared"));
    }

    #[test]
    fn health_is_drain_safe_only_without_owned_work() {
        let worker_node_id = Uuid::new_v4();
        assert!(!Health::new(worker_node_id, 0, false, JobCgroupStatus::Disabled).drain_safe);
        assert!(Health::new(worker_node_id, 0, true, JobCgroupStatus::Disabled).drain_safe);
        assert!(!Health::new(worker_node_id, 1, true, JobCgroupStatus::Disabled).drain_safe);

        let json = serde_json::to_value(Health::new(
            worker_node_id,
            2,
            true,
            JobCgroupStatus::Disabled,
        ))
        .unwrap();
        assert_eq!(json["active_execution_count"], 2);
        assert_eq!(json["admission_draining"], true);
        assert_eq!(json["drain_safe"], false);
    }

    fn required_env(coordinator_id: &str) -> Vec<(&'static str, String)> {
        vec![
            (WORKER_NODE_ID_ENV, Uuid::new_v4().to_string()),
            (WORKER_COORDINATOR_URL_ENV, "http://think2:3333".into()),
            (WORKER_COORDINATOR_ID_ENV, coordinator_id.into()),
            (
                WORKER_SIGNING_KEY_FILE_ENV,
                "/run/credentials/worker.key".into(),
            ),
            (
                COORDINATOR_PUBLIC_KEY_FILE_ENV,
                "/run/credentials/coordinator.pub".into(),
            ),
            (WORKER_EXPECTED_UID_ENV, "1000".into()),
            (WORKER_EXPECTED_GID_ENV, "100".into()),
        ]
    }

    fn parse_with(extra: &[(&'static str, &str)]) -> Result<WorkerConfig, WorkerConfigError> {
        let coordinator_id = Uuid::new_v4().to_string();
        let mut values = required_env(&coordinator_id);
        values.extend(extra.iter().map(|(key, value)| (*key, (*value).to_owned())));
        let borrowed: Vec<(&str, &str)> = values
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        parse(&borrowed)
    }

    #[test]
    fn job_cgroup_is_opt_in_and_limits_default_to_unlimited() {
        assert_eq!(parse_with(&[]).unwrap().job_cgroup, None);

        let config = parse_with(&[(WORKER_JOB_CGROUP_ENV, "jobs")]).unwrap();
        assert_eq!(
            config.job_cgroup,
            Some(JobCgroupConfig {
                name: "jobs".into(),
                memory_high: MemoryLimit::Unlimited,
                memory_max: MemoryLimit::Unlimited,
            })
        );

        let config = parse_with(&[
            (WORKER_JOB_CGROUP_ENV, "jobs"),
            (WORKER_JOB_MEMORY_HIGH_ENV, "60%"),
            (WORKER_JOB_MEMORY_MAX_ENV, "12G"),
        ])
        .unwrap();
        let job_cgroup = config.job_cgroup.unwrap();
        assert_eq!(job_cgroup.memory_high, MemoryLimit::PercentOfRam(60));
        assert_eq!(job_cgroup.memory_max, MemoryLimit::Bytes(12 << 30));
    }

    #[test]
    fn rejects_job_cgroup_paths_and_malformed_limits() {
        assert_eq!(
            parse_with(&[(WORKER_JOB_CGROUP_ENV, "../escape")]).unwrap_err(),
            WorkerConfigError::Invalid {
                name: WORKER_JOB_CGROUP_ENV,
                value: "../escape".into(),
            }
        );
        assert_eq!(
            parse_with(&[
                (WORKER_JOB_CGROUP_ENV, "jobs"),
                (WORKER_JOB_MEMORY_MAX_ENV, "12GB"),
            ])
            .unwrap_err(),
            WorkerConfigError::Invalid {
                name: WORKER_JOB_MEMORY_MAX_ENV,
                value: "12GB".into(),
            }
        );
    }

    #[test]
    fn health_reports_job_cgroup_state() {
        let json = serde_json::to_value(Health::new(
            Uuid::new_v4(),
            0,
            false,
            JobCgroupStatus::Failed {
                error: "memory controller is not delegated".into(),
            },
        ))
        .unwrap();
        assert_eq!(json["job_cgroup"]["state"], "failed");
        assert_eq!(
            json["job_cgroup"]["error"],
            "memory controller is not delegated"
        );
    }

    #[test]
    fn rejects_missing_identity_and_relative_shared_root() {
        assert_eq!(
            parse(&[
                (WORKER_COORDINATOR_URL_ENV, "http://think2:3333"),
                (WORKER_COORDINATOR_ID_ENV, &Uuid::new_v4().to_string()),
                (WORKER_SIGNING_KEY_FILE_ENV, "/run/credentials/worker.key"),
                (
                    COORDINATOR_PUBLIC_KEY_FILE_ENV,
                    "/run/credentials/coordinator.pub"
                ),
                (WORKER_EXPECTED_UID_ENV, "1000"),
                (WORKER_EXPECTED_GID_ENV, "100"),
            ]),
            Err(WorkerConfigError::Missing(WORKER_NODE_ID_ENV))
        );
        let id = Uuid::new_v4();
        assert!(matches!(
            parse(&[
                (WORKER_NODE_ID_ENV, &id.to_string()),
                (WORKER_COORDINATOR_URL_ENV, "http://think2:3333"),
                (WORKER_COORDINATOR_ID_ENV, &Uuid::new_v4().to_string()),
                (WORKER_SIGNING_KEY_FILE_ENV, "/run/credentials/worker.key"),
                (
                    COORDINATOR_PUBLIC_KEY_FILE_ENV,
                    "/run/credentials/coordinator.pub"
                ),
                (WORKER_EXPECTED_UID_ENV, "1000"),
                (WORKER_EXPECTED_GID_ENV, "100"),
                (WORKER_SHARED_ROOT_ENV, "relative"),
            ]),
            Err(WorkerConfigError::Invalid {
                name: WORKER_SHARED_ROOT_ENV,
                ..
            })
        ));
    }

    #[test]
    fn rejects_malformed_or_non_http_coordinator_urls() {
        let worker_id = Uuid::new_v4().to_string();
        let coordinator_id = Uuid::new_v4().to_string();
        for coordinator_url in [
            "http://",
            "not-a-url",
            "file:///tmp/coordinator",
            "http://user:secret@coordinator.test:3334",
        ] {
            assert!(matches!(
                parse(&[
                    (WORKER_NODE_ID_ENV, &worker_id),
                    (WORKER_COORDINATOR_URL_ENV, coordinator_url),
                    (WORKER_COORDINATOR_ID_ENV, &coordinator_id),
                    (WORKER_SIGNING_KEY_FILE_ENV, "/run/credentials/worker.key"),
                    (
                        COORDINATOR_PUBLIC_KEY_FILE_ENV,
                        "/run/credentials/coordinator.pub"
                    ),
                    (WORKER_EXPECTED_UID_ENV, "1000"),
                    (WORKER_EXPECTED_GID_ENV, "100"),
                ]),
                Err(WorkerConfigError::Invalid {
                    name: WORKER_COORDINATOR_URL_ENV,
                    ..
                })
            ));
        }
    }

    #[tokio::test]
    async fn worker_startup_removes_stale_scoped_mcp_configs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("mcp-config");
        let stale = root.join(Uuid::new_v4().to_string()).join("codex");
        tokio::fs::create_dir_all(&stale).await.unwrap();
        tokio::fs::write(stale.join("config.toml"), "bearer = 'stale'")
            .await
            .unwrap();

        reset_mcp_config_root(&root).await.unwrap();

        assert!(root.is_dir());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
    }
}
