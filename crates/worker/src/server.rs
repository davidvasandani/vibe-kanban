use std::{collections::BTreeMap, time::Duration};

use anyhow::{Context, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{
    CoordinatorLease, MountProbe, PROTOCOL_VERSION, RequestAuthority, ResourceSnapshot,
    WorkerCapabilities, WorkerHeartbeat, WorkerRegistration,
};
use ed25519_dalek::{Signer, SigningKey};
use reqwest::{Client, Method, RequestBuilder};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    WorkerConfig,
    execution::ExecutionSupervisor,
    mount_health::{MountHealthChecker, SystemMountInspector},
};

const MOUNT_CHALLENGE_PATH: &str = "/api/workers/mount-challenge";
const REGISTER_PATH: &str = "/api/workers/register";
const HEARTBEAT_PATH: &str = "/api/workers/heartbeat";
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LeaseResponse {
    lease: CoordinatorLease,
}

/// Maintains worker registration without coupling process supervision to an
/// individual HTTP connection. A failed heartbeat always returns to a fresh
/// registration attempt so coordinator restarts and expired records recover.
pub async fn registration_loop(
    config: WorkerConfig,
    supervisor: ExecutionSupervisor,
    shutdown: CancellationToken,
) {
    let signing_key = match load_signing_key(&config.signing_key_file).await {
        Ok(key) => key,
        Err(error) => {
            warn!(
                ?error,
                "worker registration disabled: signing key unavailable"
            );
            return;
        }
    };
    let client = match Client::builder().timeout(Duration::from_secs(20)).build() {
        Ok(client) => client,
        Err(error) => {
            warn!(
                ?error,
                "worker registration disabled: HTTP client unavailable"
            );
            return;
        }
    };
    let coordinator = CoordinatorClient {
        client,
        base_url: config.coordinator_url.trim_end_matches('/').to_owned(),
        signing_key,
    };

    while !shutdown.is_cancelled() {
        match coordinator.register(&config, &supervisor).await {
            Ok(lease) => {
                info!(
                    worker_node_id = %config.worker_node_id,
                    lease_expires_at = %lease.lease_expires_at,
                    "worker registered with coordinator"
                );
                if heartbeat_until_failure(&coordinator, &config, &supervisor, lease, &shutdown)
                    .await
                {
                    return;
                }
            }
            Err(error) => warn!(?error, "worker registration failed; retrying"),
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(RETRY_INTERVAL) => {}
        }
    }
}

async fn heartbeat_until_failure(
    coordinator: &CoordinatorClient,
    config: &WorkerConfig,
    supervisor: &ExecutionSupervisor,
    mut lease: CoordinatorLease,
    shutdown: &CancellationToken,
) -> bool {
    loop {
        let delay = Duration::from_secs(u64::from(lease.heartbeat_interval_seconds).max(1));
        tokio::select! {
            () = shutdown.cancelled() => return true,
            () = tokio::time::sleep(delay) => {}
        }
        match coordinator.heartbeat(config, supervisor).await {
            Ok(next_lease) => lease = next_lease,
            Err(error) => {
                warn!(?error, "worker heartbeat failed; returning to registration");
                return false;
            }
        }
    }
}

struct CoordinatorClient {
    client: Client,
    base_url: String,
    signing_key: SigningKey,
}

impl CoordinatorClient {
    async fn register(
        &self,
        config: &WorkerConfig,
        supervisor: &ExecutionSupervisor,
    ) -> anyhow::Result<CoordinatorLease> {
        let probe: MountProbe = self.get(MOUNT_CHALLENGE_PATH).await?;
        let mount = mount_health(config, &probe);
        let registration = WorkerRegistration {
            authority: authority(config),
            hostname: hostname(),
            worker_version: env!("CARGO_PKG_VERSION").to_owned(),
            vibe_version: env!("CARGO_PKG_VERSION").to_owned(),
            capabilities: WorkerCapabilities {
                executor_profiles: config.executor_profiles.clone(),
                terminal: false,
                preview: false,
                persistent_process_adoption: false,
            },
            resources: resource_snapshot(supervisor.active_execution_count().await),
            labels: BTreeMap::new(),
            mount,
        };
        let response: LeaseResponse = self.post(REGISTER_PATH, &registration).await?;
        validate_lease(response.lease)
    }

    async fn heartbeat(
        &self,
        config: &WorkerConfig,
        supervisor: &ExecutionSupervisor,
    ) -> anyhow::Result<CoordinatorLease> {
        // Fetching the probe again ensures a stale local view cannot keep a
        // worker schedulable after the coordinator changes its challenge.
        let probe: MountProbe = self.get(MOUNT_CHALLENGE_PATH).await?;
        let heartbeat = WorkerHeartbeat {
            authority: authority(config),
            resources: resource_snapshot(supervisor.active_execution_count().await),
            mount: mount_health(config, &probe),
            jobs: supervisor.inventory().await,
        };
        let response: LeaseResponse = self.post(HEARTBEAT_PATH, &heartbeat).await?;
        validate_lease(response.lease)
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        let request = self.signed(self.client.get(self.url(path)), Method::GET, path);
        decode_response(request.send().await?, path).await
    }

    async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        payload: &impl serde::Serialize,
    ) -> anyhow::Result<T> {
        let request = self.signed(self.client.post(self.url(path)), Method::POST, path);
        decode_response(request.json(payload).send().await?, path).await
    }

    fn signed(&self, request: RequestBuilder, method: Method, path: &str) -> RequestBuilder {
        let timestamp = Utc::now().timestamp();
        let message = signed_message(timestamp, &method, path);
        let signature = self.signing_key.sign(message.as_bytes());
        request
            .header("x-vk-timestamp", timestamp.to_string())
            .header(
                "x-vk-signature",
                BASE64_STANDARD.encode(signature.to_bytes()),
            )
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
}

async fn decode_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
    path: &str,
) -> anyhow::Result<T> {
    let status = response.status();
    let envelope: ApiEnvelope<T> = response
        .json()
        .await
        .with_context(|| format!("decode coordinator response from {path}"))?;
    if !status.is_success() || !envelope.success {
        bail!(
            "coordinator request {path} failed with {status}: {}",
            envelope
                .message
                .unwrap_or_else(|| "unspecified error".into())
        );
    }
    envelope
        .data
        .ok_or_else(|| anyhow!("coordinator response from {path} omitted data"))
}

async fn load_signing_key(path: &std::path::Path) -> anyhow::Result<SigningKey> {
    let encoded = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read worker signing key at {}", path.display()))?;
    let bytes = BASE64_STANDARD
        .decode(encoded.trim())
        .context("decode worker signing key as base64")?;
    let seed: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow!("worker signing key must contain exactly 32 bytes"))?;
    Ok(SigningKey::from_bytes(&seed))
}

fn signed_message(timestamp: i64, method: &Method, path: &str) -> String {
    format!("{timestamp}.{}.{path}", method.as_str())
}

fn authority(config: &WorkerConfig) -> RequestAuthority {
    RequestAuthority {
        protocol_version: PROTOCOL_VERSION,
        coordinator_id: config.coordinator_id,
        worker_node_id: config.worker_node_id,
        correlation_id: Uuid::new_v4(),
        issued_at: Utc::now(),
        nonce: Uuid::new_v4().to_string(),
    }
}

fn mount_health(config: &WorkerConfig, probe: &MountProbe) -> cluster_protocol::MountHealth {
    MountHealthChecker::new(
        &config.shared_root,
        &config.expected_export,
        config.expected_uid,
        config.expected_gid,
    )
    .check(probe, &SystemMountInspector)
}

fn validate_lease(lease: CoordinatorLease) -> anyhow::Result<CoordinatorLease> {
    if lease.accepted_protocol_version != PROTOCOL_VERSION {
        bail!(
            "coordinator accepted protocol {}, worker requires {}",
            lease.accepted_protocol_version,
            PROTOCOL_VERSION
        );
    }
    Ok(lease)
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|hostname| !hostname.trim().is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn resource_snapshot(active_execution_count: u32) -> ResourceSnapshot {
    let load_1m = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|value| value.split_whitespace().next()?.parse().ok())
        .unwrap_or(0.0);
    let available_memory_bytes = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|value| {
            value.lines().find_map(|line| {
                let kib = line
                    .strip_prefix("MemAvailable:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()?;
                kib.checked_mul(1024)
            })
        })
        .unwrap_or(0);
    ResourceSnapshot {
        cpu_count: std::thread::available_parallelism()
            .map(|count| count.get().try_into().unwrap_or(u32::MAX))
            .unwrap_or(1),
        load_1m,
        available_memory_bytes,
        active_execution_count,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use ed25519_dalek::Verifier;
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn loads_base64_seed_without_exposing_it() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("worker.key");
        tokio::fs::write(&path, BASE64_STANDARD.encode([7_u8; 32]))
            .await
            .unwrap();
        let key = load_signing_key(&path).await.unwrap();
        assert_eq!(key.to_bytes(), [7_u8; 32]);
    }

    #[tokio::test]
    async fn rejects_signing_keys_with_the_wrong_length() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("worker.key");
        tokio::fs::write(&path, BASE64_STANDARD.encode([7_u8; 31]))
            .await
            .unwrap();
        assert!(load_signing_key(&path).await.is_err());
    }

    #[test]
    fn signed_message_matches_coordinator_contract() {
        let timestamp = 1_700_000_000;
        let path = "/api/workers/register";
        let key = SigningKey::from_bytes(&[9_u8; 32]);
        let message = signed_message(timestamp, &Method::POST, path);
        let signature = key.sign(message.as_bytes());
        assert_eq!(message, "1700000000.POST./api/workers/register");
        key.verifying_key()
            .verify(message.as_bytes(), &signature)
            .unwrap();
    }

    #[test]
    fn authority_is_bound_to_configured_worker_and_coordinator() {
        let config = WorkerConfig {
            worker_node_id: Uuid::new_v4(),
            coordinator_id: Uuid::new_v4(),
            listen_addr: "127.0.0.1:8086".parse().unwrap(),
            shared_root: PathBuf::from("/shared"),
            coordinator_url: "http://coordinator".into(),
            signing_key_file: PathBuf::from("/run/key"),
            coordinator_public_key_file: PathBuf::from("/run/coordinator.pub"),
            expected_export: "server:/export".into(),
            expected_uid: 1000,
            expected_gid: 1000,
            executor_profiles: vec!["codex".into()],
            state_dir: PathBuf::from("/shared/execution-logs/worker-state/test"),
        };
        let authority = authority(&config);
        assert_eq!(authority.worker_node_id, config.worker_node_id);
        assert_eq!(authority.coordinator_id, config.coordinator_id);
        assert_eq!(authority.protocol_version, PROTOCOL_VERSION);
    }
}
