use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        FromRequestParts, Path, Request, State,
        ws::{WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{PROTOCOL_VERSION, PreviewHttpRequest, RequestAuthority};
use db::models::{
    execution_process::ExecutionProcess, execution_worker_job::ExecutionWorkerJob, session::Session,
};
use deployment::Deployment;
use ws_bridge::{bridge_axum_ws, connect_upstream_ws};

use crate::{DeploymentImpl, middleware::signed_ws::SignedWsUpgrade};

pub(super) fn api_router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/preview/{target_port}", any(proxy_preview_request_no_tail))
        .route("/preview/{target_port}/{*tail}", any(proxy_preview_request))
}

pub fn subdomain_router(deployment: DeploymentImpl) -> Router {
    Router::new()
        .fallback(subdomain_proxy_request)
        .with_state(deployment)
}

async fn proxy_preview_request_no_tail(
    State(deployment): State<DeploymentImpl>,
    Path(target_port): Path<u16>,
    ws_upgrade: Result<SignedWsUpgrade, WebSocketUpgradeRejection>,
    request: Request,
) -> Response {
    match ws_upgrade {
        Ok(ws) => forward_preview_ws(ws, target_port, String::new(), request).await,
        Err(rejection) => {
            preview_proxy::api::proxy_api_request(
                deployment.preview_proxy(),
                target_port,
                String::new(),
                Err(rejection),
                request,
            )
            .await
        }
    }
}

async fn proxy_preview_request(
    State(deployment): State<DeploymentImpl>,
    Path((target_port, tail)): Path<(u16, String)>,
    ws_upgrade: Result<SignedWsUpgrade, WebSocketUpgradeRejection>,
    request: Request,
) -> Response {
    match ws_upgrade {
        Ok(ws) => forward_preview_ws(ws, target_port, tail, request).await,
        Err(rejection) => {
            preview_proxy::api::proxy_api_request(
                deployment.preview_proxy(),
                target_port,
                tail,
                Err(rejection),
                request,
            )
            .await
        }
    }
}

async fn forward_preview_ws(
    ws: SignedWsUpgrade,
    target_port: u16,
    tail: String,
    request: Request,
) -> Response {
    let query = request.uri().query().unwrap_or_default();
    let normalized = tail.trim_start_matches('/');
    let ws_url = if normalized.is_empty() {
        format!("ws://localhost:{target_port}/?{query}")
    } else if query.is_empty() {
        format!("ws://localhost:{target_port}/{normalized}")
    } else {
        format!("ws://localhost:{target_port}/{normalized}?{query}")
    };

    let protocols = request
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);

    let (upstream_ws, selected_protocol) =
        match connect_upstream_ws(ws_url, protocols.as_deref()).await {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(?error, "Failed to connect preview upstream WebSocket");
                return (StatusCode::BAD_GATEWAY, "Preview WebSocket unavailable").into_response();
            }
        };

    let ws = if let Some(protocol) = selected_protocol {
        ws.protocols([protocol])
    } else {
        ws
    };

    ws.on_upgrade(move |client| async move {
        if let Err(error) = bridge_axum_ws(client, upstream_ws).await {
            tracing::debug!(?error, "Preview WS bridge closed with error");
        }
    })
    .into_response()
}

async fn subdomain_proxy_request(
    State(deployment): State<DeploymentImpl>,
    request: Request,
) -> Response {
    if let Some(metadata) = preview_metadata(&request) {
        return proxy_cluster_preview(&deployment, request, metadata).await;
    }
    let Some(server_addr) = deployment.client_info().get_server_addr() else {
        return (
            StatusCode::BAD_REQUEST,
            "Local server address is not available",
        )
            .into_response();
    };

    let Some(proxy_port) = deployment.client_info().get_preview_proxy_port() else {
        return (
            StatusCode::BAD_REQUEST,
            "Preview proxy port is not available",
        )
            .into_response();
    };

    preview_proxy::proxy_subdomain_request(
        deployment.preview_proxy(),
        server_addr,
        proxy_port,
        request,
    )
    .await
}

struct ClusterPreviewMetadata {
    workspace_id: uuid::Uuid,
    execution_id: uuid::Uuid,
    generation: u64,
}

fn preview_metadata(request: &Request) -> Option<ClusterPreviewMetadata> {
    if let Some(query) = request.uri().query() {
        let values = url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        if let (Some(workspace), Some(execution), Some(generation)) = (
            values.get("_vk_workspace"),
            values.get("_vk_execution"),
            values.get("_vk_generation"),
        ) {
            return Some(ClusterPreviewMetadata {
                workspace_id: workspace.parse().ok()?,
                execution_id: execution.parse().ok()?,
                generation: generation.parse().ok()?,
            });
        }
    }
    let host = request.headers().get("host")?.to_str().ok()?;
    let labels = host.split('.').collect::<Vec<_>>();
    let marker = labels.iter().position(|label| *label == "vk")?;
    Some(ClusterPreviewMetadata {
        workspace_id: labels.get(marker + 1)?.parse().ok()?,
        execution_id: labels.get(marker + 2)?.parse().ok()?,
        generation: labels.get(marker + 3)?.parse().ok()?,
    })
}

async fn proxy_cluster_preview(
    deployment: &DeploymentImpl,
    request: Request,
    metadata: ClusterPreviewMetadata,
) -> Response {
    let Some(client) = deployment.worker_client() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Worker client unavailable").into_response();
    };
    let Ok(Some(process)) =
        ExecutionProcess::find_by_id(&deployment.db().pool, metadata.execution_id).await
    else {
        return (StatusCode::GONE, "Preview execution is no longer available").into_response();
    };
    let Ok(Some(session)) = Session::find_by_id(&deployment.db().pool, process.session_id).await
    else {
        return (StatusCode::GONE, "Preview session is no longer available").into_response();
    };
    if session.workspace_id != metadata.workspace_id
        || process.started_at.timestamp_millis() as u64 != metadata.generation
    {
        return (StatusCode::GONE, "Stale preview generation").into_response();
    }
    let Ok(Some(job)) =
        ExecutionWorkerJob::find_by_execution_id(&deployment.db().pool, metadata.execution_id)
            .await
    else {
        return (StatusCode::GONE, "Preview worker job is unavailable").into_response();
    };
    let Some(coordinator_id) = deployment.cluster_config().coordinator_id else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Coordinator identity unavailable",
        )
            .into_response();
    };
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let Some(port) = host
        .split('.')
        .next()
        .and_then(|label| label.split("--").next())
        .and_then(|value| value.parse::<u16>().ok())
    else {
        return (StatusCode::BAD_REQUEST, "Invalid preview port").into_response();
    };
    let (mut parts, body) = request.into_parts();
    let query = parts
        .uri
        .query()
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .filter(|(key, _)| !key.starts_with("_vk_"))
                .fold(
                    url::form_urlencoded::Serializer::new(String::new()),
                    |mut serializer, (key, value)| {
                        serializer.append_pair(&key, &value);
                        serializer
                    },
                )
                .finish()
        })
        .unwrap_or_default();
    let path_and_query = if query.is_empty() {
        parts.uri.path().to_string()
    } else {
        format!("{}?{query}", parts.uri.path())
    };
    let headers = parts
        .headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect();
    let authority = RequestAuthority {
        protocol_version: PROTOCOL_VERSION,
        coordinator_id,
        worker_node_id: job.worker_node_id,
        correlation_id: metadata.execution_id,
        issued_at: Utc::now(),
        nonce: uuid::Uuid::new_v4().to_string(),
    };
    let ws_upgrade = WebSocketUpgrade::from_request_parts(&mut parts, &())
        .await
        .ok();
    if let Some(ws) = ws_upgrade {
        let payload = PreviewHttpRequest {
            authority,
            workspace_id: metadata.workspace_id,
            execution_id: metadata.execution_id,
            worker_job_id: job.worker_job_id,
            generation: metadata.generation,
            port,
            method: "GET".into(),
            path_and_query,
            headers,
            body_base64: String::new(),
        };
        return match client.preview_websocket(job.worker_node_id, &payload).await {
            Ok((upstream, selected_protocol)) => {
                let ws = if let Some(protocol) = selected_protocol {
                    ws.protocols([protocol])
                } else {
                    ws
                };
                ws.on_upgrade(move |browser| async move {
                    if let Err(error) = bridge_axum_ws(browser, upstream).await {
                        tracing::debug!("cluster preview WebSocket closed: {error}");
                    }
                })
                .into_response()
            }
            Err(error) => {
                tracing::warn!(execution_id = %metadata.execution_id, "cluster preview WebSocket failed: {error}");
                (StatusCode::BAD_GATEWAY, "Preview WebSocket unavailable").into_response()
            }
        };
    }
    let body = match to_bytes(body, 50 * 1024 * 1024).await {
        Ok(body) => body,
        Err(_) => {
            return (StatusCode::PAYLOAD_TOO_LARGE, "Preview request too large").into_response();
        }
    };
    let payload = PreviewHttpRequest {
        authority,
        workspace_id: metadata.workspace_id,
        execution_id: metadata.execution_id,
        worker_job_id: job.worker_job_id,
        generation: metadata.generation,
        port,
        method: parts.method.to_string(),
        path_and_query,
        headers,
        body_base64: BASE64_STANDARD.encode(body),
    };
    match client.proxy_preview(job.worker_node_id, &payload).await {
        Ok(upstream) => {
            let mut response = Response::builder().status(upstream.status);
            for (name, value) in upstream.headers {
                response = response.header(name, value);
            }
            match BASE64_STANDARD.decode(upstream.body_base64) {
                Ok(body) => response
                    .body(Body::from(body))
                    .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()),
                Err(_) => (StatusCode::BAD_GATEWAY, "Invalid preview response").into_response(),
            }
        }
        Err(error) => {
            tracing::warn!(execution_id = %metadata.execution_id, "cluster preview proxy failed: {error}");
            (StatusCode::BAD_GATEWAY, "Preview upstream unavailable").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_metadata_requires_complete_cluster_identity() {
        let workspace_id = uuid::Uuid::new_v4();
        let execution_id = uuid::Uuid::new_v4();
        let request = Request::builder()
            .uri(format!(
                "/?_vk_workspace={workspace_id}&_vk_execution={execution_id}&_vk_generation=42"
            ))
            .body(Body::empty())
            .unwrap();
        let metadata = preview_metadata(&request).unwrap();
        assert_eq!(metadata.workspace_id, workspace_id);
        assert_eq!(metadata.execution_id, execution_id);
        assert_eq!(metadata.generation, 42);

        let incomplete = Request::builder()
            .uri(format!("/?_vk_workspace={workspace_id}"))
            .body(Body::empty())
            .unwrap();
        assert!(preview_metadata(&incomplete).is_none());

        let host_routed = Request::builder()
            .uri("/asset.js")
            .header(
                "host",
                format!("3000.vk.{workspace_id}.{execution_id}.42.localhost:40775"),
            )
            .body(Body::empty())
            .unwrap();
        let metadata = preview_metadata(&host_routed).unwrap();
        assert_eq!(metadata.workspace_id, workspace_id);
        assert_eq!(metadata.execution_id, execution_id);
        assert_eq!(metadata.generation, 42);
    }
}
