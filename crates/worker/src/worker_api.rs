use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path, Query, Request, State, ws::WebSocketUpgrade},
    http::StatusCode,
    middleware::{Next, from_fn_with_state},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{
    CancellationRequest, DispatchAccepted, EventAcknowledgement, EventBatch, ExecutionDispatch,
    InteractionResponse, JobSummary, PROTOCOL_VERSION, PreviewHttpRequest, PreviewHttpResponse,
    QuarantineRequest, RequestAuthority, TerminalClose, TerminalCreateRequest, TerminalCreated,
    TerminalInput, TerminalOutputBatch, TerminalResize,
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
    preview::PreviewService,
    terminal::TerminalService,
};

const SIGNATURE_HEADER: &str = "x-vk-signature";
const TIMESTAMP_HEADER: &str = "x-vk-timestamp";
const CONTENT_DIGEST_HEADER: &str = "x-vk-content-sha256";
// Preview requests allow 50 MiB of raw body data at the coordinator. The
// signed worker envelope carries that body base64-encoded inside JSON, so keep
// enough headroom for the 4/3 expansion and protocol metadata.
const MAX_SIGNED_BODY_BYTES: usize = 72 * 1024 * 1024;
const MAX_TIMESTAMP_DRIFT_SECONDS: i64 = 30;
const NONCE_RETENTION: Duration = Duration::from_secs(120);

#[derive(Clone)]
struct WorkerApiState {
    supervisor: ExecutionSupervisor,
    worker_node_id: Uuid,
    coordinator_id: Uuid,
    coordinator_key: VerifyingKey,
    seen_nonces: Arc<Mutex<HashMap<String, tokio::time::Instant>>>,
    terminals: TerminalService,
    preview: PreviewService,
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    #[serde(default)]
    after: u64,
}

#[derive(Debug, Deserialize)]
struct PreviewWsQuery {
    workspace_id: Uuid,
    worker_job_id: Uuid,
    path_and_query: String,
    protocols: Option<String>,
}

#[derive(Debug, Serialize)]
struct Acknowledged {
    highest_contiguous_sequence: u64,
}

pub async fn router(
    config: &WorkerConfig,
    supervisor: ExecutionSupervisor,
) -> anyhow::Result<Router> {
    let coordinator_key = load_verifying_key(&config.coordinator_public_key_file).await?;
    let path_authority = PathAuthority::new(&config.shared_root)?;
    let state = WorkerApiState {
        supervisor,
        worker_node_id: config.worker_node_id,
        coordinator_id: config.coordinator_id,
        coordinator_key,
        seen_nonces: Arc::new(Mutex::new(HashMap::new())),
        terminals: TerminalService::new(path_authority),
        preview: PreviewService::new(),
    };
    Ok(Router::new()
        .route("/v1/jobs", get(inventory))
        .route("/v1/terminals", post(create_terminal))
        .route("/v1/terminals/{terminal_id}/output", get(terminal_output))
        .route("/v1/terminals/{terminal_id}/input", post(terminal_input))
        .route("/v1/terminals/{terminal_id}/resize", post(terminal_resize))
        .route("/v1/terminals/{terminal_id}/close", post(close_terminal))
        .route(
            "/v1/executions/{execution_id}/preview/{generation}/{port}",
            post(proxy_preview),
        )
        .route(
            "/v1/executions/{execution_id}/preview/{generation}/{port}/ws",
            get(proxy_preview_ws),
        )
        .route("/v1/executions/{execution_id}", post(dispatch))
        .route("/v1/executions/{execution_id}/events", get(events))
        .route("/v1/executions/{execution_id}/ack", post(acknowledge))
        .route("/v1/executions/{execution_id}/cancel", post(cancel))
        .route(
            "/v1/executions/{execution_id}/interactions/{interaction_id}",
            post(respond_interaction),
        )
        .route("/v1/executions/{execution_id}/quarantine", post(quarantine))
        .layer(from_fn_with_state(state.clone(), require_signature))
        .with_state(state))
}

async fn proxy_preview_ws(
    State(state): State<WorkerApiState>,
    Path((execution_id, generation, port)): Path<(Uuid, u64, u16)>,
    Query(query): Query<PreviewWsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, WorkerApiError> {
    if generation == 0
        || port == 0
        || !query.path_and_query.starts_with('/')
        || !state
            .supervisor
            .authorizes_preview(execution_id, query.workspace_id, query.worker_job_id)
            .await
    {
        return Err(WorkerApiError::Forbidden);
    }
    let upstream_url = format!("ws://127.0.0.1:{port}{}", query.path_and_query);
    let (upstream, selected_protocol) =
        ws_bridge::connect_upstream_ws(upstream_url, query.protocols.as_deref())
            .await
            .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    let ws = if let Some(protocol) = selected_protocol {
        ws.protocols([protocol])
    } else {
        ws
    };
    Ok(ws.on_upgrade(move |client| async move {
        if let Err(error) = ws_bridge::bridge_axum_ws(client, upstream).await {
            tracing::debug!("preview websocket closed: {error}");
        }
    }))
}

async fn proxy_preview(
    State(state): State<WorkerApiState>,
    Path((execution_id, generation, port)): Path<(Uuid, u64, u16)>,
    Json(payload): Json<PreviewHttpRequest>,
) -> Result<Json<PreviewHttpResponse>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id
        || payload.generation != generation
        || payload.port != port
        || payload.authority.correlation_id != execution_id
    {
        return Err(WorkerApiError::BadRequest("preview target mismatch".into()));
    }
    if !state
        .supervisor
        .authorizes_preview(execution_id, payload.workspace_id, payload.worker_job_id)
        .await
    {
        return Err(WorkerApiError::Forbidden);
    }
    state
        .preview
        .proxy(payload)
        .await
        .map(Json)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))
}

async fn create_terminal(
    State(state): State<WorkerApiState>,
    Json(payload): Json<TerminalCreateRequest>,
) -> Result<Json<TerminalCreated>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.authority.correlation_id != payload.workspace_id {
        return Err(WorkerApiError::BadRequest(
            "terminal authority is not bound to workspace".into(),
        ));
    }
    let terminal_id = state
        .terminals
        .create(payload)
        .await
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    Ok(Json(TerminalCreated { terminal_id }))
}

async fn terminal_output(
    State(state): State<WorkerApiState>,
    Path(terminal_id): Path<Uuid>,
) -> Result<Json<TerminalOutputBatch>, WorkerApiError> {
    state
        .terminals
        .output(terminal_id)
        .map(Json)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))
}

async fn terminal_input(
    State(state): State<WorkerApiState>,
    Path(terminal_id): Path<Uuid>,
    Json(payload): Json<TerminalInput>,
) -> Result<Json<serde_json::Value>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.terminal_id != terminal_id {
        return Err(WorkerApiError::BadRequest("terminal ID mismatch".into()));
    }
    let bytes = BASE64_STANDARD
        .decode(payload.data_base64)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    state
        .terminals
        .input(terminal_id, &bytes)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    Ok(Json(serde_json::json!({ "acknowledged": true })))
}

async fn terminal_resize(
    State(state): State<WorkerApiState>,
    Path(terminal_id): Path<Uuid>,
    Json(payload): Json<TerminalResize>,
) -> Result<Json<serde_json::Value>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.terminal_id != terminal_id {
        return Err(WorkerApiError::BadRequest("terminal ID mismatch".into()));
    }
    state
        .terminals
        .resize(terminal_id, payload.cols, payload.rows)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    Ok(Json(serde_json::json!({ "acknowledged": true })))
}

async fn close_terminal(
    State(state): State<WorkerApiState>,
    Path(terminal_id): Path<Uuid>,
    Json(payload): Json<TerminalClose>,
) -> Result<Json<serde_json::Value>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.terminal_id != terminal_id {
        return Err(WorkerApiError::BadRequest("terminal ID mismatch".into()));
    }
    state
        .terminals
        .close(terminal_id)
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    Ok(Json(serde_json::json!({ "acknowledged": true })))
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

async fn respond_interaction(
    State(state): State<WorkerApiState>,
    Path((execution_id, interaction_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<InteractionResponse>,
) -> Result<Json<serde_json::Value>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id || payload.interaction_id != interaction_id {
        return Err(WorkerApiError::BadRequest(
            "path IDs do not match interaction response".into(),
        ));
    }
    let outcome = serde_json::from_str(&payload.response).map_err(|error| {
        WorkerApiError::BadRequest(format!("invalid interaction response: {error}"))
    })?;
    if !state
        .supervisor
        .respond_interaction(execution_id, interaction_id, outcome)
        .await?
    {
        return Err(WorkerApiError::BadRequest(
            "interaction is not pending".into(),
        ));
    }
    Ok(Json(serde_json::json!({ "acknowledged": true })))
}

async fn quarantine(
    State(state): State<WorkerApiState>,
    Path(execution_id): Path<Uuid>,
    Json(payload): Json<QuarantineRequest>,
) -> Result<Json<JobSummary>, WorkerApiError> {
    validate_authority(&state, &payload.authority).await?;
    if payload.execution_id != execution_id {
        return Err(WorkerApiError::BadRequest(
            "path execution ID does not match quarantine request".into(),
        ));
    }
    tracing::warn!(%execution_id, reason = %payload.reason, "Quarantining worker job");
    let cancellation = CancellationRequest {
        authority: payload.authority,
        execution_id,
        graceful_timeout_seconds: 0,
        terminate_timeout_seconds: 0,
    };
    cancellation::cancel(&state.supervisor, &cancellation)
        .await
        .map_err(|error| WorkerApiError::BadRequest(error.to_string()))?;
    state
        .supervisor
        .quarantine(execution_id)
        .await
        .map(Json)
        .map_err(Into::into)
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
