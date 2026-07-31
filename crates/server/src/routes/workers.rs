use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path, Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Json as ResponseJson, Response},
    routing::{get, patch, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{CoordinatorLease, MountProbe, WorkerHeartbeat, WorkerRegistration};
use db::models::worker_node::WorkerNode;
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::cluster::{MountChallenge, WorkerRegistryError};
use sha2::{Digest, Sha256};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Serialize)]
struct RegistrationResponse {
    worker: WorkerNode,
    lease: CoordinatorLease,
}

#[derive(Debug, Deserialize)]
struct DrainWorkerRequest {
    draining: bool,
}

pub fn admin_router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/worker-nodes", get(list_workers))
        .route("/worker-nodes/{worker_node_id}", patch(set_draining))
}

pub fn worker_router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route("/workers/mount-challenge", get(get_mount_challenge))
        .route("/workers/register", post(register))
        .route("/workers/heartbeat", post(heartbeat))
        .layer(from_fn_with_state(
            deployment.clone(),
            require_worker_signature,
        ))
}

async fn list_workers(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<WorkerNode>>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(
        WorkerNode::fetch_all(&deployment.db().pool).await?,
    )))
}

async fn set_draining(
    State(deployment): State<DeploymentImpl>,
    Path(worker_node_id): Path<Uuid>,
    Json(payload): Json<DrainWorkerRequest>,
) -> Result<ResponseJson<ApiResponse<WorkerNode>>, ApiError> {
    if !deployment
        .worker_registry()
        .set_draining(worker_node_id, payload.draining)
        .await
        .map_err(registry_error)?
    {
        return Err(ApiError::BadRequest("Worker node not found".into()));
    }
    let worker = WorkerNode::find_by_id(&deployment.db().pool, worker_node_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Worker node not found".into()))?;
    Ok(ResponseJson(ApiResponse::success(worker)))
}

async fn get_mount_challenge(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<MountProbe>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(
        mount_challenge(&deployment).await?.probe,
    )))
}

async fn register(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<WorkerRegistration>,
) -> Result<ResponseJson<ApiResponse<RegistrationResponse>>, ApiError> {
    claim_nonce(&deployment, &payload.authority.nonce).await?;
    let (worker, lease) = deployment
        .worker_registry()
        .register(&payload, &mount_challenge(&deployment).await?, Utc::now())
        .await
        .map_err(registry_error)?;
    Ok(ResponseJson(ApiResponse::success(RegistrationResponse {
        worker,
        lease,
    })))
}

async fn heartbeat(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<WorkerHeartbeat>,
) -> Result<ResponseJson<ApiResponse<RegistrationResponse>>, ApiError> {
    claim_nonce(&deployment, &payload.authority.nonce).await?;
    let (worker, lease) = deployment
        .worker_registry()
        .heartbeat(&payload, &mount_challenge(&deployment).await?, Utc::now())
        .await
        .map_err(registry_error)?;
    Ok(ResponseJson(ApiResponse::success(RegistrationResponse {
        worker,
        lease,
    })))
}

async fn mount_challenge(deployment: &DeploymentImpl) -> Result<MountChallenge, ApiError> {
    let config = deployment.cluster_config();
    if !config.enabled {
        return Err(ApiError::BadRequest("Cluster mode is disabled".into()));
    }
    let coordinator_id = config
        .coordinator_id
        .ok_or_else(|| ApiError::BadRequest("Cluster coordinator ID is missing".into()))?;
    let id = coordinator_id.to_string();
    let relative_path = format!(".coordinator-probes/{id}");
    let probe_path = config.shared_root.join(&relative_path);
    let contents = format!("vibe-kanban-coordinator:{id}\n");
    if let Some(parent) = probe_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&probe_path, contents.as_bytes()).await?;
    let digest = format!("{:x}", Sha256::digest(contents.as_bytes()));
    Ok(MountChallenge {
        probe: MountProbe {
            id: id.clone(),
            relative_path,
            expected_contents_digest: digest,
        },
        expected_filesystem_id: config.expected_filesystem_id.clone(),
    })
}

async fn claim_nonce(deployment: &DeploymentImpl, nonce: &str) -> Result<(), ApiError> {
    deployment
        .trusted_key_auth()
        .claim_refresh_nonce(nonce)
        .await
        .map_err(|_| ApiError::Unauthorized)
}

async fn require_worker_signature(
    State(deployment): State<DeploymentImpl>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 4 * 1024 * 1024)
        .await
        .map_err(|_| ApiError::Unauthorized)?;
    let computed_digest = BASE64_STANDARD.encode(Sha256::digest(&body));
    let supplied_digest = parts
        .headers
        .get("x-vk-content-sha256")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::Unauthorized)?;
    if supplied_digest != computed_digest {
        return Err(ApiError::Unauthorized);
    }
    deployment
        .trusted_key_auth()
        .verify_request_signature_with_content_digest(
            &parts.headers,
            &parts.method,
            parts.uri.path(),
            &computed_digest,
        )
        .await
        .map_err(|error| {
            tracing::warn!(?error, "rejected worker request signature");
            ApiError::Unauthorized
        })?;
    Ok(next
        .run(Request::from_parts(parts, Body::from(body)))
        .await
        .into_response())
}

fn registry_error(error: WorkerRegistryError) -> ApiError {
    match error {
        WorkerRegistryError::Database(error) => ApiError::Database(error),
        WorkerRegistryError::UnsupportedProtocol { .. }
        | WorkerRegistryError::WrongCoordinator { .. }
        | WorkerRegistryError::InvalidMountEvidence(_)
        | WorkerRegistryError::WorkerNotRegistered(_)
        | WorkerRegistryError::MissingCoordinatorId
        | WorkerRegistryError::Serialization(_) => ApiError::BadRequest(error.to_string()),
    }
}
