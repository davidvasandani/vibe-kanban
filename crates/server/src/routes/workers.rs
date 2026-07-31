use axum::{
    Json, Router,
    extract::{Path, Request, State},
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Json as ResponseJson, Response},
    routing::{get, patch, post},
};
use chrono::Utc;
use cluster_protocol::{CoordinatorLease, MountProbe, WorkerHeartbeat, WorkerRegistration};
use db::models::worker_node::WorkerNode;
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::cluster::{MountChallenge, WorkerRegistryError};
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
        mount_challenge(&deployment)?.probe,
    )))
}

async fn register(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<WorkerRegistration>,
) -> Result<ResponseJson<ApiResponse<RegistrationResponse>>, ApiError> {
    claim_nonce(&deployment, &payload.authority.nonce).await?;
    let (worker, lease) = deployment
        .worker_registry()
        .register(&payload, &mount_challenge(&deployment)?, Utc::now())
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
        .heartbeat(&payload, &mount_challenge(&deployment)?, Utc::now())
        .await
        .map_err(registry_error)?;
    Ok(ResponseJson(ApiResponse::success(RegistrationResponse {
        worker,
        lease,
    })))
}

fn mount_challenge(deployment: &DeploymentImpl) -> Result<MountChallenge, ApiError> {
    let config = deployment.cluster_config();
    if !config.enabled {
        return Err(ApiError::BadRequest("Cluster mode is disabled".into()));
    }
    let coordinator_id = config
        .coordinator_id
        .ok_or_else(|| ApiError::BadRequest("Cluster coordinator ID is missing".into()))?;
    let id = coordinator_id.to_string();
    Ok(MountChallenge {
        probe: MountProbe {
            id: id.clone(),
            relative_path: format!(".coordinator-probes/{id}"),
            expected_contents_digest: format!("coordinator:{id}"),
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
    deployment
        .trusted_key_auth()
        .verify_request_signature(request.headers(), request.method(), request.uri().path())
        .await
        .map_err(|error| {
            tracing::warn!(?error, "rejected worker request signature");
            ApiError::Unauthorized
        })?;
    Ok(next.run(request).await.into_response())
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
