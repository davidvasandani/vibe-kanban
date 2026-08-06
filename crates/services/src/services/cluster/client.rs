use std::{collections::HashMap, sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use cluster_protocol::{
    CancellationRequest, CancellationStatus, CodexRolloutArtifact, CodexRolloutManifest,
    CodexRolloutManifestRequest, CodexRolloutReadRequest, CodexRolloutStageRequest,
    CodexRolloutStageResult, CodexRolloutVerification, CodexRolloutVerifyRequest,
    DispatchAccepted, EventAcknowledgement, EventBatch, ExecutionQuiescenceRequest,
    ExecutionQuiescenceStatus,
    ExecutionDispatch, InteractionResponse, JobSummary, PreviewHttpRequest, PreviewHttpResponse,
    QuarantineRequest, TerminalClose, TerminalCreateRequest, TerminalCreated, TerminalInput,
    TerminalOutputBatch, TerminalResize,
};
use ed25519_dalek::{Signer, SigningKey};
use futures::StreamExt;
use node_metrics::SampleBatch;
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WorkerClientError {
    #[error("worker {0} has no configured reachable endpoint")]
    EndpointNotFound(Uuid),
    #[error("worker endpoint URL could not be constructed: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("worker transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("worker WebSocket transport failed: {0}")]
    WebSocket(String),
    #[error("worker rejected request with status {status}: {message}")]
    Rejected { status: StatusCode, message: String },
    #[error("worker reported a dispatch digest conflict")]
    DigestConflict,
    #[error(
        "worker event replay gap: requested after {requested_after}, earliest retained is {earliest_available}"
    )]
    ReplayGap {
        requested_after: u64,
        earliest_available: u64,
    },
    /// The worker answered `404`: its build predates the route. Distinct from
    /// [`WorkerClientError::Rejected`] so a caller can report
    /// `NotImplemented` rather than `Unreachable` — one is a version skew, the
    /// other is a fault, and conflating them makes an old worker look broken.
    #[error("worker does not implement {path}")]
    NotImplemented { path: String },
    /// The reply exceeded the caller's explicit cap. Buffering it would let a
    /// misbehaving worker size the coordinator's heap.
    #[error("worker response exceeded {limit} bytes")]
    ResponseTooLarge { limit: usize },
    #[error("worker response could not be decoded: {0}")]
    MalformedResponse(String),
}

/// Far below the client's 30s default: metrics are polled on a 2s tick, so a
/// node that has not answered in 5s has already missed its slot.
const METRICS_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// A full `after=0` fetch is ~150 samples with one process table, which is
/// well under a megabyte in practice. The cap exists so a misbehaving worker
/// cannot size the coordinator's heap, not to constrain honest replies.
const MAX_METRICS_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct WorkerHealth {
    worker_node_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct WorkerErrorBody {
    error: Option<String>,
}

#[derive(Clone)]
pub struct WorkerClient {
    http: Client,
    configured_endpoints: Arc<Vec<Url>>,
    endpoint_cache: Arc<RwLock<HashMap<Uuid, Url>>>,
    signing_key: Arc<SigningKey>,
}

impl WorkerClient {
    pub fn new(endpoints: Vec<Url>, signing_key: SigningKey) -> Result<Self, WorkerClientError> {
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(30)).build()?,
            configured_endpoints: Arc::new(endpoints),
            endpoint_cache: Arc::new(RwLock::new(HashMap::new())),
            signing_key: Arc::new(signing_key),
        })
    }

    pub async fn dispatch(
        &self,
        worker_node_id: Uuid,
        dispatch: &ExecutionDispatch,
    ) -> Result<DispatchAccepted, WorkerClientError> {
        let path = format!("/v1/executions/{}", dispatch.execution_id);
        let mut payload = dispatch.clone();
        for attempt in 0..2 {
            if attempt > 0 {
                refresh_dispatch_authority(&mut payload, chrono::Utc::now(), Uuid::new_v4());
            }
            match self.post(worker_node_id, &path, &payload).await {
                Err(error) if attempt == 0 && retryable_dispatch_error(&error) => {}
                result => return result,
            }
        }
        unreachable!("bounded dispatch retry returns on final attempt")
    }

    pub async fn codex_rollout_manifest(
        &self,
        worker_node_id: Uuid,
        request: &CodexRolloutManifestRequest,
    ) -> Result<CodexRolloutManifest, WorkerClientError> {
        let path = format!("/v1/session-transfers/{}/manifest", request.operation_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn codex_rollout_artifact(
        &self,
        worker_node_id: Uuid,
        request: &CodexRolloutReadRequest,
    ) -> Result<CodexRolloutArtifact, WorkerClientError> {
        let path = format!("/v1/session-transfers/{}/artifact", request.manifest.operation_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn stage_codex_rollout(
        &self,
        worker_node_id: Uuid,
        request: &CodexRolloutStageRequest,
    ) -> Result<CodexRolloutStageResult, WorkerClientError> {
        let path = format!("/v1/session-transfers/{}/stage", request.manifest.operation_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn verify_codex_rollouts(
        &self,
        worker_node_id: Uuid,
        request: &CodexRolloutVerifyRequest,
    ) -> Result<CodexRolloutVerification, WorkerClientError> {
        let path = format!("/v1/session-transfers/{}/verify", request.manifest.operation_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn set_execution_quiesced(
        &self,
        worker_node_id: Uuid,
        request: &ExecutionQuiescenceRequest,
        quiesced: bool,
    ) -> Result<ExecutionQuiescenceStatus, WorkerClientError> {
        let action = if quiesced { "quiesce" } else { "resume" };
        let path = format!("/v1/session-transfers/{}/{action}", request.operation_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn events(
        &self,
        worker_node_id: Uuid,
        execution_id: Uuid,
        after: u64,
    ) -> Result<EventBatch, WorkerClientError> {
        let path = format!("/v1/executions/{execution_id}/events");
        let signed_target = format!("{path}?after={after}");
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let url = endpoint.join(&signed_target)?;
        let response = self
            .signed(self.http.get(url), Method::GET, &signed_target, &[])
            .send()
            .await?;
        let batch: EventBatch = decode(response).await?;
        validate_event_batch(&batch)?;
        Ok(batch)
    }

    pub async fn acknowledge(
        &self,
        worker_node_id: Uuid,
        acknowledgement: &EventAcknowledgement,
    ) -> Result<u64, WorkerClientError> {
        #[derive(Deserialize)]
        struct Acknowledged {
            highest_contiguous_sequence: u64,
        }
        let path = format!("/v1/executions/{}/ack", acknowledgement.execution_id);
        let response: Acknowledged = self.post(worker_node_id, &path, acknowledgement).await?;
        Ok(response.highest_contiguous_sequence)
    }

    pub async fn cancel(
        &self,
        worker_node_id: Uuid,
        cancellation: &CancellationRequest,
    ) -> Result<CancellationStatus, WorkerClientError> {
        let path = format!("/v1/executions/{}/cancel", cancellation.execution_id);
        self.post(worker_node_id, &path, cancellation).await
    }

    pub async fn quarantine(
        &self,
        worker_node_id: Uuid,
        request: &QuarantineRequest,
    ) -> Result<JobSummary, WorkerClientError> {
        let path = format!("/v1/executions/{}/quarantine", request.execution_id);
        self.post(worker_node_id, &path, request).await
    }

    pub async fn respond_interaction(
        &self,
        worker_node_id: Uuid,
        response: &InteractionResponse,
    ) -> Result<(), WorkerClientError> {
        let path = format!(
            "/v1/executions/{}/interactions/{}",
            response.execution_id, response.interaction_id
        );
        let _: serde_json::Value = self.post(worker_node_id, &path, response).await?;
        Ok(())
    }

    pub async fn create_terminal(
        &self,
        worker_node_id: Uuid,
        request: &TerminalCreateRequest,
    ) -> Result<TerminalCreated, WorkerClientError> {
        self.post(worker_node_id, "/v1/terminals", request).await
    }

    pub async fn proxy_preview(
        &self,
        worker_node_id: Uuid,
        request: &PreviewHttpRequest,
    ) -> Result<PreviewHttpResponse, WorkerClientError> {
        let path = format!(
            "/v1/executions/{}/preview/{}/{}",
            request.execution_id, request.generation, request.port
        );
        self.post(worker_node_id, &path, request).await
    }

    pub async fn preview_websocket(
        &self,
        worker_node_id: Uuid,
        request: &PreviewHttpRequest,
    ) -> Result<
        (
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Option<String>,
        ),
        WorkerClientError,
    > {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let path = format!(
            "/v1/executions/{}/preview/{}/{}/ws",
            request.execution_id, request.generation, request.port
        );
        let query = preview_ws_query(request);
        let signed_target = format!("{path}?{query}");
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let http_url = endpoint.join(&signed_target)?;
        let signed = self
            .signed(
                self.http.get(http_url.clone()),
                Method::GET,
                &signed_target,
                &[],
            )
            .build()?;
        let mut ws_url = http_url;
        ws_url
            .set_scheme(if ws_url.scheme() == "https" {
                "wss"
            } else {
                "ws"
            })
            .map_err(|_| WorkerClientError::WebSocket("invalid worker WebSocket URL".into()))?;
        let mut ws_request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|error| WorkerClientError::WebSocket(error.to_string()))?;
        for (name, value) in signed.headers() {
            ws_request.headers_mut().insert(name, value.clone());
        }
        let (stream, response) = tokio_tungstenite::connect_async(ws_request)
            .await
            .map_err(|error| WorkerClientError::WebSocket(error.to_string()))?;
        let selected_protocol = response
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        Ok((stream, selected_protocol))
    }

    pub async fn terminal_output(
        &self,
        worker_node_id: Uuid,
        terminal_id: Uuid,
    ) -> Result<TerminalOutputBatch, WorkerClientError> {
        let path = format!("/v1/terminals/{terminal_id}/output");
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let response = self
            .signed(
                self.http.get(endpoint.join(&path)?),
                Method::GET,
                &path,
                &[],
            )
            .send()
            .await?;
        decode(response).await
    }

    pub async fn terminal_input(
        &self,
        worker_node_id: Uuid,
        request: &TerminalInput,
    ) -> Result<(), WorkerClientError> {
        let path = format!("/v1/terminals/{}/input", request.terminal_id);
        let _: serde_json::Value = self.post(worker_node_id, &path, request).await?;
        Ok(())
    }

    pub async fn terminal_resize(
        &self,
        worker_node_id: Uuid,
        request: &TerminalResize,
    ) -> Result<(), WorkerClientError> {
        let path = format!("/v1/terminals/{}/resize", request.terminal_id);
        let _: serde_json::Value = self.post(worker_node_id, &path, request).await?;
        Ok(())
    }

    pub async fn close_terminal(
        &self,
        worker_node_id: Uuid,
        request: &TerminalClose,
    ) -> Result<(), WorkerClientError> {
        let path = format!("/v1/terminals/{}/close", request.terminal_id);
        let _: serde_json::Value = self.post(worker_node_id, &path, request).await?;
        Ok(())
    }

    pub async fn inventory(
        &self,
        worker_node_id: Uuid,
    ) -> Result<Vec<JobSummary>, WorkerClientError> {
        let path = "/v1/jobs";
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let response = self
            .signed(self.http.get(endpoint.join(path)?), Method::GET, path, &[])
            .send()
            .await?;
        decode(response).await
    }

    /// Host metrics with `sequence > after`.
    ///
    /// Same shape as [`WorkerClient::inventory`] — a signed bodyless `GET` —
    /// with three deliberate differences, all of them consequences of running
    /// on a 2s tick rather than on demand:
    ///
    /// - **A 5s per-request timeout**, not the client's 30s default. A hung
    ///   node must not stall the polls queued behind it.
    /// - **The cursor is inside the signed target**, so a captured signature
    ///   cannot be replayed against a different `after`.
    /// - **An explicit response cap**, applied while streaming rather than
    ///   after buffering.
    ///
    /// There is deliberately **no nonce header**: `require_signature` on the
    /// worker never consults one for a bodyless route, and sending a header the
    /// peer does not check would be security theatre. See analysis E2.
    pub async fn metrics(
        &self,
        worker_node_id: Uuid,
        after: u64,
    ) -> Result<SampleBatch, WorkerClientError> {
        let path = "/v1/metrics";
        let signed_target = format!("{path}?after={after}");
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let url = endpoint.join(&signed_target)?;
        let response = self
            // A fresh envelope per call: the worker rejects anything outside a
            // ±30s drift window, so a cached one starts failing within half a
            // minute of being built.
            .signed(
                self.http.get(url).timeout(METRICS_REQUEST_TIMEOUT),
                Method::GET,
                &signed_target,
                &[],
            )
            .send()
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(WorkerClientError::NotImplemented {
                path: path.to_owned(),
            });
        }
        let body = capped_body(response, MAX_METRICS_RESPONSE_BYTES).await?;
        serde_json::from_slice(&body)
            .map_err(|error| WorkerClientError::MalformedResponse(error.to_string()))
    }

    async fn post<T: DeserializeOwned>(
        &self,
        worker_node_id: Uuid,
        path: &str,
        payload: &impl Serialize,
    ) -> Result<T, WorkerClientError> {
        let endpoint = self.endpoint_for(worker_node_id).await?;
        let body = serde_json::to_vec(payload).expect("protocol payload must serialize");
        let request = self.signed(
            self.http
                .post(endpoint.join(path)?)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.clone()),
            Method::POST,
            path,
            &body,
        );
        decode(request.send().await?).await
    }

    async fn endpoint_for(&self, worker_node_id: Uuid) -> Result<Url, WorkerClientError> {
        if let Some(endpoint) = self.endpoint_cache.read().await.get(&worker_node_id) {
            return Ok(endpoint.clone());
        }
        for endpoint in self.configured_endpoints.iter() {
            let health_url = endpoint.join("/health")?;
            let Ok(response) = self.http.get(health_url).send().await else {
                continue;
            };
            let Ok(health) = response.json::<WorkerHealth>().await else {
                continue;
            };
            self.endpoint_cache
                .write()
                .await
                .insert(health.worker_node_id, endpoint.clone());
            if health.worker_node_id == worker_node_id {
                return Ok(endpoint.clone());
            }
        }
        Err(WorkerClientError::EndpointNotFound(worker_node_id))
    }

    fn signed(
        &self,
        request: reqwest::RequestBuilder,
        method: Method,
        path: &str,
        body: &[u8],
    ) -> reqwest::RequestBuilder {
        let timestamp = chrono::Utc::now().timestamp();
        let content_digest = BASE64_STANDARD.encode(Sha256::digest(body));
        let message = format!("{timestamp}.{}.{path}.{content_digest}", method.as_str());
        let signature = self.signing_key.sign(message.as_bytes());
        request
            .header("x-vk-timestamp", timestamp.to_string())
            .header("x-vk-content-sha256", content_digest)
            .header(
                "x-vk-signature",
                BASE64_STANDARD.encode(signature.to_bytes()),
            )
    }
}

fn refresh_dispatch_authority(
    dispatch: &mut ExecutionDispatch,
    issued_at: chrono::DateTime<chrono::Utc>,
    nonce: Uuid,
) {
    dispatch.authority.issued_at = issued_at;
    dispatch.authority.nonce = nonce.to_string();
}

fn retryable_dispatch_error(error: &WorkerClientError) -> bool {
    match error {
        WorkerClientError::Transport(_) => true,
        WorkerClientError::Rejected { status, .. } => {
            status.is_server_error()
                || *status == StatusCode::REQUEST_TIMEOUT
                || *status == StatusCode::TOO_MANY_REQUESTS
        }
        _ => false,
    }
}

fn preview_ws_query(request: &PreviewHttpRequest) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer
        .append_pair("workspace_id", &request.workspace_id.to_string())
        .append_pair("worker_job_id", &request.worker_job_id.to_string())
        .append_pair("path_and_query", &request.path_and_query);
    if let Some(protocols) = request.headers.get("sec-websocket-protocol") {
        serializer.append_pair("protocols", protocols);
    }
    serializer.finish()
}

async fn decode<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, WorkerClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response.json().await?);
    }
    if status == StatusCode::CONFLICT {
        return Err(WorkerClientError::DigestConflict);
    }
    let message = response
        .json::<WorkerErrorBody>()
        .await
        .ok()
        .and_then(|body| body.error)
        .unwrap_or_else(|| "unspecified worker error".into());
    Err(WorkerClientError::Rejected { status, message })
}

/// Buffer a response body, refusing to exceed `limit`.
///
/// The check is applied *while* streaming and against the advertised
/// `Content-Length` first, so an oversized reply is abandoned rather than
/// allocated and then rejected.
async fn capped_body(
    response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, WorkerClientError> {
    let status = response.status();
    if !status.is_success() {
        let message = response
            .json::<WorkerErrorBody>()
            .await
            .ok()
            .and_then(|body| body.error)
            .unwrap_or_else(|| "unspecified worker error".into());
        return Err(WorkerClientError::Rejected { status, message });
    }
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(WorkerClientError::ResponseTooLarge { limit });
    }
    let mut buffer = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if buffer.len() + chunk.len() > limit {
            return Err(WorkerClientError::ResponseTooLarge { limit });
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

fn validate_event_batch(batch: &EventBatch) -> Result<(), WorkerClientError> {
    if batch.replay_gap {
        return Err(WorkerClientError::ReplayGap {
            requested_after: batch.requested_after,
            earliest_available: batch.earliest_available,
        });
    }
    let mut expected = batch.requested_after.saturating_add(1);
    for event in &batch.events {
        if event.sequence != expected {
            return Err(WorkerClientError::ReplayGap {
                requested_after: expected.saturating_sub(1),
                earliest_available: event.sequence,
            });
        }
        expected = expected.saturating_add(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;
    use cluster_protocol::{
        ExecutionEvent, ExecutionEventPayload, PROTOCOL_VERSION, PersistencePolicy,
        RequestAuthority,
    };

    use super::*;

    fn batch(sequences: &[u64]) -> EventBatch {
        EventBatch {
            execution_id: Uuid::new_v4(),
            requested_after: 3,
            earliest_available: sequences.first().copied().unwrap_or(4),
            latest_available: sequences.last().copied().unwrap_or(3),
            replay_gap: false,
            events: sequences
                .iter()
                .map(|sequence| ExecutionEvent {
                    execution_id: Uuid::nil(),
                    sequence: *sequence,
                    worker_timestamp: Utc::now(),
                    payload: ExecutionEventPayload::Accepted,
                })
                .collect(),
        }
    }

    #[test]
    fn accepts_contiguous_events_after_cursor() {
        assert!(validate_event_batch(&batch(&[4, 5, 6])).is_ok());
    }

    #[test]
    fn rejects_worker_reported_or_observed_replay_gap() {
        let mut reported = batch(&[7]);
        reported.replay_gap = true;
        assert!(matches!(
            validate_event_batch(&reported),
            Err(WorkerClientError::ReplayGap { .. })
        ));
        assert!(matches!(
            validate_event_batch(&batch(&[4, 6])),
            Err(WorkerClientError::ReplayGap { .. })
        ));
    }

    #[test]
    fn dispatch_retry_refreshes_only_replay_authority() {
        let execution_id = Uuid::new_v4();
        let original_time = Utc::now() - chrono::TimeDelta::seconds(1);
        let mut dispatch = ExecutionDispatch {
            authority: RequestAuthority {
                protocol_version: PROTOCOL_VERSION,
                coordinator_id: Uuid::new_v4(),
                worker_node_id: Uuid::new_v4(),
                correlation_id: execution_id,
                issued_at: original_time,
                nonce: "first".into(),
            },
            execution_id,
            workspace_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            workspace_path: "/shared/workspace".into(),
            working_directory: "/shared/workspace".into(),
            executor_profile: "codex".into(),
            executor_profile_config: None,
            action: serde_json::json!({}),
            environment: BTreeMap::new(),
            run_reason: "test".into(),
            timeout_seconds: None,
            persistence: PersistencePolicy::Ordinary,
            request_digest: "stable-digest".into(),
        };
        let original = dispatch.clone();
        let retry_time = Utc::now();
        let retry_nonce = Uuid::new_v4();

        refresh_dispatch_authority(&mut dispatch, retry_time, retry_nonce);

        assert_eq!(dispatch.execution_id, original.execution_id);
        assert_eq!(dispatch.request_digest, original.request_digest);
        assert_eq!(
            dispatch.authority.correlation_id,
            original.authority.correlation_id
        );
        assert_eq!(dispatch.authority.issued_at, retry_time);
        assert_eq!(dispatch.authority.nonce, retry_nonce.to_string());
    }

    #[test]
    fn dispatch_retries_only_transient_rejections() {
        let rejected = |status| WorkerClientError::Rejected {
            status,
            message: "test".into(),
        };
        assert!(retryable_dispatch_error(&rejected(
            StatusCode::INTERNAL_SERVER_ERROR
        )));
        assert!(retryable_dispatch_error(&rejected(
            StatusCode::REQUEST_TIMEOUT
        )));
        assert!(retryable_dispatch_error(&rejected(
            StatusCode::TOO_MANY_REQUESTS
        )));
        assert!(!retryable_dispatch_error(&rejected(StatusCode::FORBIDDEN)));
        assert!(!retryable_dispatch_error(
            &WorkerClientError::DigestConflict
        ));
    }
}
