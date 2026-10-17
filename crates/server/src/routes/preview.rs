use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        FromRequestParts, Path, Request, State,
        ws::{WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use cluster_protocol::{PROTOCOL_VERSION, PreviewHttpRequest, RequestAuthority};
use db::models::{
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    execution_worker_job::ExecutionWorkerJob,
    preview_lease::PreviewLease,
    session::Session,
};
use deployment::Deployment;
use sha2::{Digest, Sha256};
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
    if is_public_preview_request(&request) {
        return public_preview_request(&deployment, request).await;
    }
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

const PREVIEW_COOKIE: &str = "__Host-vk-preview";
const PREVIEW_OPEN_PATH: &str = "/__vk/open";
const PREVIEW_PROXY_ERROR: &str = "x-vk-preview-proxy-error";

fn public_preview_host() -> Option<String> {
    let value = std::env::var("VK_PUBLIC_PREVIEW_URL").ok()?;
    url::Url::parse(&value)
        .ok()?
        .host_str()
        .map(ToOwned::to_owned)
}

fn is_public_preview_request(request: &Request) -> bool {
    let Some(expected) = public_preview_host() else {
        return false;
    };
    request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(':').next())
        .is_some_and(|host| {
            host.eq_ignore_ascii_case(&expected)
                || host
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{}", expected.to_ascii_lowercase()))
        })
}

fn public_preview_lease_id(request: &Request) -> Option<uuid::Uuid> {
    let base = public_preview_host()?;
    let host = request
        .headers()
        .get(header::HOST)?
        .to_str()
        .ok()?
        .split(':')
        .next()?;
    host.strip_suffix(&format!(".{base}"))?.parse().ok()
}

fn token_digest(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn cookie_token(request: &Request) -> Option<&str> {
    request
        .headers()
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|item| item.strip_prefix(&format!("{PREVIEW_COOKIE}=")))
}

fn upstream_header(name: &str, value: &str) -> Option<String> {
    if !name.eq_ignore_ascii_case("cookie") {
        return Some(value.to_string());
    }
    let filtered = value
        .split(';')
        .map(str::trim)
        .filter(|item| !item.starts_with(&format!("{PREVIEW_COOKIE}=")))
        .collect::<Vec<_>>()
        .join("; ");
    (!filtered.is_empty()).then_some(filtered)
}

fn downstream_header(name: &str, value: &str, target_port: u16) -> Option<String> {
    if name.eq_ignore_ascii_case("set-cookie")
        && value
            .split('=')
            .next()
            .is_some_and(|cookie_name| cookie_name.trim() == PREVIEW_COOKIE)
    {
        return None;
    }
    if name.eq_ignore_ascii_case("location")
        && let Ok(url) = url::Url::parse(value)
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        && url.port_or_known_default() == Some(target_port)
    {
        let mut relative = url.path().to_string();
        if let Some(query) = url.query() {
            relative.push('?');
            relative.push_str(query);
        }
        if let Some(fragment) = url.fragment() {
            relative.push('#');
            relative.push_str(fragment);
        }
        return Some(relative);
    }
    Some(value.to_string())
}

fn bootstrap_page() -> Response {
    let html = r#"<!doctype html><meta charset="utf-8"><title>Opening preview…</title>
<script>
(async()=>{const token=location.hash.slice(1);if(!token){document.body.textContent='Preview capability missing or expired.';return}
const response=await fetch('/__vk/authorize',{method:'POST',headers:{'content-type':'text/plain'},body:token});
if(response.ok){history.replaceState(null,'','/');location.reload()}else{document.body.textContent=await response.text()}})();
</script><noscript>JavaScript is required to open this temporary preview.</noscript>"#;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(
            "content-security-policy",
            "default-src 'none'; script-src 'unsafe-inline'; connect-src 'self'",
        )
        .body(Body::from(html))
        .unwrap()
}

async fn public_preview_request(deployment: &DeploymentImpl, request: Request) -> Response {
    let Some(host_lease_id) = public_preview_lease_id(&request) else {
        return (StatusCode::NOT_FOUND, "Preview lease origin not found").into_response();
    };
    if request.uri().path() == PREVIEW_OPEN_PATH && request.method() == axum::http::Method::GET {
        return bootstrap_page();
    }
    if request.uri().path() == "/__vk/authorize" && request.method() == axum::http::Method::POST {
        let body = match to_bytes(request.into_body(), 256).await {
            Ok(body) => body,
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "Invalid preview capability").into_response();
            }
        };
        let Ok(token) = std::str::from_utf8(&body) else {
            return (StatusCode::BAD_REQUEST, "Invalid preview capability").into_response();
        };
        let lease = PreviewLease::find_by_digest(&deployment.db().pool, &token_digest(token)).await;
        let Ok(Some(lease)) = lease else {
            return (StatusCode::NOT_FOUND, "Preview capability not found").into_response();
        };
        if lease.id != host_lease_id || lease.is_expired(Utc::now()) {
            return (
                StatusCode::GONE,
                "Preview capability expired or was revoked",
            )
                .into_response();
        }
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(
                header::SET_COOKIE,
                format!(
                    "{PREVIEW_COOKIE}={token}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={}",
                    (lease.expires_at - Utc::now()).num_seconds().max(0)
                ),
            )
            .header(header::CACHE_CONTROL, "no-store")
            .body(Body::empty())
            .unwrap();
    }

    let Some(token) = cookie_token(&request) else {
        return if request.uri().path() == "/" {
            bootstrap_page()
        } else {
            (StatusCode::UNAUTHORIZED, "Preview capability required").into_response()
        };
    };
    let lease =
        match PreviewLease::find_by_digest(&deployment.db().pool, &token_digest(token)).await {
            Ok(Some(lease)) => lease,
            _ => return (StatusCode::NOT_FOUND, "Preview capability not found").into_response(),
        };
    if lease.id != host_lease_id || lease.is_expired(Utc::now()) {
        return (
            StatusCode::GONE,
            "Preview capability expired or was revoked",
        )
            .into_response();
    }
    let Ok(Some(process)) =
        ExecutionProcess::find_by_id(&deployment.db().pool, lease.execution_process_id).await
    else {
        return (StatusCode::GONE, "Preview forwarding session ended").into_response();
    };
    if process.status != ExecutionProcessStatus::Running {
        return (StatusCode::GONE, "Preview forwarding session ended").into_response();
    }
    let metadata = ClusterPreviewMetadata {
        workspace_id: lease.workspace_id,
        execution_id: lease.execution_process_id,
        generation: process.started_at.timestamp_millis() as u64,
    };
    proxy_cluster_preview_on_port(
        deployment,
        request,
        metadata,
        lease.target_port as u16,
        Some(lease),
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
    proxy_cluster_preview_on_port(deployment, request, metadata, port, None).await
}

fn preview_proxy_error(status: StatusCode, message: &'static str) -> Response {
    Response::builder()
        .status(status)
        .header(PREVIEW_PROXY_ERROR, "1")
        .body(Body::from(message))
        .unwrap()
}

async fn wait_for_preview_lease_end(pool: sqlx::SqlitePool, lease: PreviewLease) {
    loop {
        let active = PreviewLease::find_by_id(&pool, lease.id)
            .await
            .ok()
            .flatten()
            .is_some_and(|current| !current.is_expired(Utc::now()));
        let process_running = ExecutionProcess::find_by_id(&pool, lease.execution_process_id)
            .await
            .ok()
            .flatten()
            .is_some_and(|process| process.status == ExecutionProcessStatus::Running);
        if !active || !process_running {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn proxy_cluster_preview_on_port(
    deployment: &DeploymentImpl,
    request: Request,
    metadata: ClusterPreviewMetadata,
    port: u16,
    lease_guard: Option<PreviewLease>,
) -> Response {
    let Some(client) = deployment.worker_client() else {
        return preview_proxy_error(StatusCode::SERVICE_UNAVAILABLE, "Worker client unavailable");
    };
    let Ok(Some(process)) =
        ExecutionProcess::find_by_id(&deployment.db().pool, metadata.execution_id).await
    else {
        return preview_proxy_error(StatusCode::GONE, "Preview execution is no longer available");
    };
    let Ok(Some(session)) = Session::find_by_id(&deployment.db().pool, process.session_id).await
    else {
        return preview_proxy_error(StatusCode::GONE, "Preview session is no longer available");
    };
    if session.workspace_id != metadata.workspace_id
        || process.started_at.timestamp_millis() as u64 != metadata.generation
    {
        return preview_proxy_error(StatusCode::GONE, "Stale preview generation");
    }
    let Ok(Some(job)) =
        ExecutionWorkerJob::find_by_execution_id(&deployment.db().pool, metadata.execution_id)
            .await
    else {
        return preview_proxy_error(StatusCode::GONE, "Preview worker job is unavailable");
    };
    let Some(coordinator_id) = deployment.cluster_config().coordinator_id else {
        return preview_proxy_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Coordinator identity unavailable",
        );
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
            value.to_str().ok().and_then(|value| {
                upstream_header(name.as_str(), value).map(|value| (name.to_string(), value))
            })
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
                let pool = deployment.db().pool.clone();
                ws.on_upgrade(move |browser| async move {
                    let bridge = bridge_axum_ws(browser, upstream);
                    if let Some(lease) = lease_guard {
                        tokio::select! {
                            result = bridge => {
                                if let Err(error) = result {
                                    tracing::debug!("cluster preview WebSocket closed: {error}");
                                }
                            }
                            _ = wait_for_preview_lease_end(pool, lease) => {}
                        }
                    } else if let Err(error) = bridge.await {
                        tracing::debug!("cluster preview WebSocket closed: {error}");
                    }
                })
                .into_response()
            }
            Err(error) => {
                tracing::warn!(execution_id = %metadata.execution_id, "cluster preview WebSocket failed: {error}");
                preview_proxy_error(StatusCode::BAD_GATEWAY, "Preview WebSocket unavailable")
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
                if lease_guard.is_some()
                    && name.eq_ignore_ascii_case(header::CACHE_CONTROL.as_str())
                {
                    continue;
                }
                if let Some(value) = downstream_header(&name, &value, port) {
                    response = response.header(name, value);
                }
            }
            if lease_guard.is_some() {
                response = response.header(header::CACHE_CONTROL, "no-store");
            }
            match BASE64_STANDARD.decode(upstream.body_base64) {
                Ok(body) => response
                    .body(Body::from(body))
                    .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response()),
                Err(_) => preview_proxy_error(StatusCode::BAD_GATEWAY, "Invalid preview response"),
            }
        }
        Err(error) => {
            tracing::warn!(execution_id = %metadata.execution_id, "cluster preview proxy failed: {error}");
            preview_proxy_error(StatusCode::BAD_GATEWAY, "Preview upstream unavailable")
        }
    }
}

pub enum PreviewProbeResult {
    Ready,
    ForwardingUnavailable,
    DevServerUnavailable,
}

pub async fn probe_preview_lease(
    deployment: &DeploymentImpl,
    lease: &PreviewLease,
) -> PreviewProbeResult {
    let mut forwarding_failed = false;
    for _ in 0..4 {
        let process =
            ExecutionProcess::find_by_id(&deployment.db().pool, lease.execution_process_id)
                .await
                .ok()
                .flatten();
        if let Some(process) = process {
            let request = Request::builder().uri("/").body(Body::empty()).unwrap();
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                proxy_cluster_preview_on_port(
                    deployment,
                    request,
                    ClusterPreviewMetadata {
                        workspace_id: lease.workspace_id,
                        execution_id: lease.execution_process_id,
                        generation: process.started_at.timestamp_millis() as u64,
                    },
                    lease.target_port as u16,
                    None,
                ),
            )
            .await;
            let Ok(response) = response else {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                continue;
            };
            if !response.headers().contains_key(PREVIEW_PROXY_ERROR) {
                return PreviewProbeResult::Ready;
            }
            forwarding_failed |= matches!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE | StatusCode::GONE
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }
    if forwarding_failed {
        PreviewProbeResult::ForwardingUnavailable
    } else {
        PreviewProbeResult::DevServerUnavailable
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

    #[test]
    fn preview_cookie_extracts_only_the_named_capability() {
        let request = Request::builder()
            .uri("/")
            .header(
                header::COOKIE,
                "other=x; __Host-vk-preview=secret-token; last=y",
            )
            .body(Body::empty())
            .unwrap();
        assert_eq!(cookie_token(&request), Some("secret-token"));
        assert_eq!(token_digest("secret-token").len(), 64);
        assert_eq!(
            upstream_header("cookie", "other=x; __Host-vk-preview=secret-token; last=y"),
            Some("other=x; last=y".into())
        );
        assert_eq!(
            upstream_header("cookie", "__Host-vk-preview=secret-token"),
            None
        );
        assert_eq!(
            downstream_header("location", "http://127.0.0.1:4173/search?q=x", 4173),
            Some("/search?q=x".into())
        );
        assert_eq!(
            downstream_header("set-cookie", "__Host-vk-preview=attacker", 4173),
            None
        );
    }

    #[test]
    fn bootstrap_keeps_capability_in_the_fragment() {
        let response = bootstrap_page();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}
