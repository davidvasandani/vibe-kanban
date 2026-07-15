use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, HeaderName, Method, StatusCode},
    response::{IntoResponse, Response},
};
use db::models::mcp_gateway::McpGatewayConnection;
use futures_util::TryStreamExt;
use reqwest::redirect::Policy;

use super::StoredCredentials;

const FORWARD_REQUEST: &[&str] = &[
    "accept", "content-type", "mcp-protocol-version", "mcp-session-id", "last-event-id",
];
const FORWARD_RESPONSE: &[&str] = &[
    "content-type", "cache-control", "mcp-protocol-version", "mcp-session-id", "retry-after",
];

pub async fn forward(
    request: Request,
    connection: &McpGatewayConnection,
    credentials: StoredCredentials,
) -> Response {
    if !matches!(*request.method(), Method::GET | Method::POST | Method::DELETE) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let (parts, body) = request.into_parts();
    let client = match reqwest::Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let method = match reqwest::Method::from_bytes(parts.method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return StatusCode::METHOD_NOT_ALLOWED.into_response(),
    };
    let mut upstream = client.request(method, &connection.upstream_url);
    upstream = copy_headers(upstream, &parts.headers, FORWARD_REQUEST);
    upstream = upstream.bearer_auth(credentials.access_token);
    if let (Some(id), Some(secret)) = (
        credentials.cf_access_client_id,
        credentials.cf_access_client_secret,
    ) {
        upstream = upstream
            .header("CF-Access-Client-Id", id)
            .header("CF-Access-Client-Secret", secret);
    }
    let request_stream = body.into_data_stream().map_err(std::io::Error::other);
    let upstream = match upstream.body(reqwest::Body::wrap_stream(request_stream)).send().await {
        Ok(response) => response,
        Err(_) => return (StatusCode::BAD_GATEWAY, "Upstream MCP server is unavailable").into_response(),
    };
    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    for name in FORWARD_RESPONSE {
        if let Some(value) = upstream.headers().get(*name) {
            response = response.header(*name, value);
        }
    }
    match response.body(Body::from_stream(upstream.bytes_stream())) {
        Ok(response) => response,
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

fn copy_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
    allowlist: &[&str],
) -> reqwest::RequestBuilder {
    for name in allowlist {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else { continue };
        if let Some(value) = headers.get(&header_name) {
            builder = builder.header(name.to_string(), value.as_bytes());
        }
    }
    builder
}
