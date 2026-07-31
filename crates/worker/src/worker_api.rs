use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{
    CancellationRequest, DispatchAccepted, EventAcknowledgement, EventBatch, ExecutionDispatch,
    JobSummary, PROTOCOL_VERSION, RequestAuthority,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    WorkerConfig, cancellation,
    execution::{ExecutionError, ExecutionSupervisor},
    path_authority::PathAuthority,
};

const SIGNATURE_HEADER: &str = "x-vk-signature";
const TIMESTAMP_HEADER: &str = "x-vk-timestamp";
const CONTENT_DIGEST_HEADER: &str = "x-vk-content-sha256";
const MAX_SIGNED_BODY_BYTES: usize = 1024 * 1024;
const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;
const NONCE_RETENTION: Duration = Duration::from_secs(120);

#[derive(Clone)]
struct WorkerApiState {
    supervisor: ExecutionSupervisor,
    worker_node_id: Uuid,
    coordinator_id: Uuid,
    coordinator_key: VerifyingKey,
    seen_nonces: Arc<Mutex<HashMap<String, tokio::time::Instant>>>,
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    #[serde(default)]
    after: u64,
}

#[derive(Debug, Serialize)]
struct Acknowledged {
    highest_contiguous_sequence: u64,
}

pub async fn router(config: &WorkerConfig) -> anyhow::Result<Router> {
    let coordinator_key = load_verifying_key(&config.coordinator_public_key_file).await?;
    let supervisor = ExecutionSupervisor::new(PathAuthority::new(&config.shared_root)?);
    let state = WorkerApiState {
        supervisor,
        worker_node_id: config.worker_node_id,
        coordinator_id: config.coordinator_id,
        coordinator_key,
        seen_nonces: Arc::new(Mutex::new(HashMap::new())),
    };
    Ok(Router::new()
        .route("/v1/jobs", get(inventory))
        .route("/v1/executions/{execution_id}", post(dispatch))
        .route("/v1/executions/{execution_id}/events", get(events))
        .route("/v1/executions/{execution_id}/ack", post(acknowledge))
        .route("/v1/executions/{execution_id}/cancel", post(cancel))
        .layer(from_fn_with_state(state.clone(), require_signature))
        .with_state(state))
}

async fn inventory(State(state): State<WorkerApiState>) -> Json<Vec<JobSummary>> {
    Json(state.supervisor.inventory().await)
}

async fn dispatch(
    State(state): State<WorkerApiState>,
    Path(execution_id): Path<Uuid>,
    Json(payload): Json<ExecutionDispatch>,
) -> Result<Json<DispatchAccepted>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id {
        return Err(WorkerApiError::BadRequest(
            "path execution ID does not match dispatch".into(),
        ));
    }
    state
        .supervisor
        .dispatch(payload)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn events(
    State(state): State<WorkerApiState>,
    Path(execution_id): Path<Uuid>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<EventBatch>, WorkerApiError> {
    state
        .supervisor
        .events(execution_id, query.after)
        .await
        .map(Json)
        .map_err(Into::into)
}

async fn acknowledge(
    State(state): State<WorkerApiState>,
    Path(execution_id): Path<Uuid>,
    Json(payload): Json<EventAcknowledgement>,
) -> Result<Json<Acknowledged>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id {
        return Err(WorkerApiError::BadRequest(
            "path execution ID does not match acknowledgement".into(),
        ));
    }
    let highest_contiguous_sequence = state
        .supervisor
        .acknowledge(execution_id, payload.highest_contiguous_sequence)
        .await?;
    Ok(Json(Acknowledged {
        highest_contiguous_sequence,
    }))
}

async fn cancel(
    State(state): State<WorkerApiState>,
    Path(execution_id): Path<Uuid>,
    Json(payload): Json<CancellationRequest>,
) -> Result<Json<cluster_protocol::CancellationStatus>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id {
        return Err(WorkerApiError::BadRequest(
            "path execution ID does not match cancellation".into(),
        ));
    }
    cancellation::cancel(&state.supervisor, &payload)
        .await
        .map(Json)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))
}

async fn validate_authority(
    state: &WorkerApiState,
    authority: &RequestAuthority,
) -> Result<(), WorkerApiError> {
    if authority.protocol_version != PROTOCOL_VERSION
        || authority.worker_node_id != state.worker_node_id
        || authority.coordinator_id != state.coordinator_id
    {
        return Err(WorkerApiError::Forbidden);
    }
    let now = Utc::now();
    if (now - authority.issued_at).num_seconds().abs() > MAX_TIMESTAMP_DRIFT_SECONDS {
        return Err(WorkerApiError::Forbidden);
    }
    let mut nonces = state.seen_nonces.lock().await;
    nonces.retain(|_, seen| seen.elapsed() <= NONCE_RETENTION);
    if nonces.contains_key(&authority.nonce) {
        return Err(WorkerApiError::Forbidden);
    }
    nonces.insert(authority.nonce.clone(), tokio::time::Instant::now());
    Ok(())
}

async fn require_signature(
    State(state): State<WorkerApiState>,
    request: Request,
    next: Next,
) -> Result<Response, WorkerApiError> {
    let timestamp = request
        .headers()
        .get(TIMESTAMP_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
        .ok_or(WorkerApiError::Unauthorized)?;
    if Utc::now().timestamp().saturating_sub(timestamp).abs() > MAX_TIMESTAMP_DRIFT_SECONDS {
        return Err(WorkerApiError::Unauthorized);
    }
    let signature = request
        .headers()
        .get(SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| BASE64_STANDARD.decode(value).ok())
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .map(|bytes| Signature::from_bytes(&bytes))
        .ok_or(WorkerApiError::Unauthorized)?;
    let content_digest = request
        .headers()
        .get(CONTENT_DIGEST_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(WorkerApiError::Unauthorized)?
        .to_owned();
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, MAX_SIGNED_BODY_BYTES)
        .await
        .map_err(|_| WorkerApiError::Unauthorized)?;
    let computed_digest = BASE64_STANDARD.encode(Sha256::digest(&body));
    if content_digest != computed_digest {
        return Err(WorkerApiError::Unauthorized);
    }
    let message = format!(
        "{timestamp}.{}.{}.{}",
        parts.method.as_str(),
        parts.uri.path(),
        content_digest
    );
    state
        .coordinator_key
        .verify_strict(message.as_bytes(), &signature)
        .map_err(|_| WorkerApiError::Unauthorized)?;
    Ok(next
        .run(Request::from_parts(parts, Body::from(body)))
        .await
        .into_response())
}

async fn load_verifying_key(path: &std::path::Path) -> anyhow::Result<VerifyingKey> {
    let encoded = tokio::fs::read_to_string(path).await?;
    let bytes = BASE64_STANDARD.decode(encoded.trim())?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("coordinator public key must contain exactly 32 bytes"))?;
    Ok(VerifyingKey::from_bytes(&bytes)?)
}

enum WorkerApiError {
    Unauthorized,
    Forbidden,
    BadRequest(String),
    Execution(ExecutionError),
}

impl From<ExecutionError> for WorkerApiError {
    fn from(error: ExecutionError) -> Self {
        Self::Execution(error)
    }
}

impl IntoResponse for WorkerApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "invalid request signature".into()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "request authority rejected".into()),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Execution(ExecutionError::DigestConflict { .. }) => {
                (StatusCode::CONFLICT, "execution digest conflict".into())
            }
            Self::Execution(error) => (StatusCode::BAD_REQUEST, error.to_string()),
        };
        (status, Json(serde_json::json!({"error": message}))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;

    use super::*;

    #[tokio::test]
    async fn loads_base64_coordinator_public_key() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("coordinator.pub");
        let expected = SigningKey::from_bytes(&[31_u8; 32]).verifying_key();
        tokio::fs::write(&path, BASE64_STANDARD.encode(expected.as_bytes()))
            .await
            .unwrap();
        assert_eq!(load_verifying_key(&path).await.unwrap(), expected);
    }
}
