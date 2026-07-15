use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use axum::{
    body::{Body, Bytes, to_bytes},
    extract::Request,
    http::{HeaderMap, HeaderName, Method, StatusCode},
    response::{IntoResponse, Response},
};
use db::models::mcp_gateway::McpGatewayConnection;
use reqwest::redirect::Policy;

use super::StoredCredentials;

const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const FORWARD_REQUEST: &[&str] = &[
    "accept",
    "content-type",
    "mcp-protocol-version",
    "mcp-session-id",
    "last-event-id",
];
const FORWARD_RESPONSE: &[&str] = &[
    "content-type",
    "cache-control",
    "mcp-protocol-version",
    "mcp-session-id",
    "retry-after",
];

#[derive(Clone)]
pub struct PreparedRequest {
    method: Method,
    headers: HeaderMap,
    body: Bytes,
}

pub async fn prepare(request: Request) -> Result<PreparedRequest, Response> {
    if !matches!(
        *request.method(),
        Method::GET | Method::POST | Method::DELETE
    ) {
        return Err(StatusCode::METHOD_NOT_ALLOWED.into_response());
    }
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, MAX_REQUEST_BYTES)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE.into_response())?;
    Ok(PreparedRequest {
        method: parts.method,
        headers: parts.headers,
        body,
    })
}

pub async fn forward(
    request: &PreparedRequest,
    connection: &McpGatewayConnection,
    credentials: &StoredCredentials,
) -> Response {
    let client = match validated_client(&connection.upstream_url).await {
        Ok(client) => client,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let method = match reqwest::Method::from_bytes(request.method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return StatusCode::METHOD_NOT_ALLOWED.into_response(),
    };
    let mut upstream = client.request(method, &connection.upstream_url);
    upstream = copy_headers(upstream, &request.headers, FORWARD_REQUEST);
    upstream = upstream.bearer_auth(&credentials.access_token);
    if let (Some(id), Some(secret)) = (
        credentials.cf_access_client_id.as_deref(),
        credentials.cf_access_client_secret.as_deref(),
    ) {
        upstream = upstream
            .header("CF-Access-Client-Id", id)
            .header("CF-Access-Client-Secret", secret);
    }
    let upstream = match upstream.body(request.body.clone()).send().await {
        Ok(response) => response,
        Err(_) => {
            return (
                StatusCode::BAD_GATEWAY,
                "Upstream MCP server is unavailable",
            )
                .into_response();
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
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

async fn validated_client(raw_url: &str) -> Result<reqwest::Client, ()> {
    let url = reqwest::Url::parse(raw_url).map_err(|_| ())?;
    let host = url
        .host_str()
        .ok_or(())?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = url.port_or_known_default().ok_or(())?;
    let addresses: Vec<SocketAddr> = if let Ok(ip) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(ip, port)]
    } else {
        tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|_| ())?
            .collect()
    };
    if addresses.is_empty() {
        return Err(());
    }
    let loopback_http = url.scheme() == "http" && addresses.iter().all(|a| a.ip().is_loopback());
    if !loopback_http
        && (url.scheme() != "https" || addresses.iter().any(|a| !is_public_ip(a.ip())))
    {
        return Err(());
    }
    reqwest::Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .resolve_to_addrs(&host, &addresses)
        .build()
        .map_err(|_| ())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && b == 168)
                || (a == 192 && b == 0 && c == 2)
                || (a == 198 && (b == 18 || b == 19))
                || (a == 198 && b == 51 && c == 100)
                || (a == 203 && b == 0 && c == 113)
                || a >= 224)
        }
        IpAddr::V6(ip) => {
            !(ip == Ipv6Addr::UNSPECIFIED
                || ip == Ipv6Addr::LOCALHOST
                || ip.is_multicast()
                || (ip.segments()[0] & 0xfe00) == 0xfc00
                || (ip.segments()[0] & 0xffc0) == 0xfe80
                || ip
                    .to_ipv4_mapped()
                    .is_some_and(|v4| !is_public_ip(IpAddr::V4(v4))))
        }
    }
}

fn copy_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &HeaderMap,
    allowlist: &[&str],
) -> reqwest::RequestBuilder {
    for name in allowlist {
        let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        if let Some(value) = headers.get(&header_name) {
            builder = builder.header(name.to_string(), value.as_bytes());
        }
    }
    builder
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::to_bytes;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };

    use super::*;

    #[tokio::test]
    async fn forwards_mcp_headers_and_streaming_body_but_replaces_authorization() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(String::new()));
        let captured_task = captured.clone();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 8192];
            let count = socket.read(&mut bytes).await.unwrap();
            *captured_task.lock().await = String::from_utf8_lossy(&bytes[..count]).into_owned();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nmcp-session-id: s-2\r\ntransfer-encoding: chunked\r\n\r\n9\r\ndata: 1\n\n\r\n9\r\ndata: 2\n\n\r\n0\r\n\r\n")
                .await
                .unwrap();
        });
        let request = PreparedRequest {
            method: Method::POST,
            headers: HeaderMap::from_iter([
                (
                    axum::http::header::AUTHORIZATION,
                    "Bearer attacker".parse().unwrap(),
                ),
                (
                    HeaderName::from_static("mcp-session-id"),
                    "s-1".parse().unwrap(),
                ),
            ]),
            body: Bytes::from_static(b"{}"),
        };
        let connection = McpGatewayConnection {
            id: "id".into(),
            owner_user_id: "u".into(),
            machine_id: "m".into(),
            server_name: "s".into(),
            upstream_url: format!("http://{addr}/mcp"),
            transport: "http".into(),
            auth_kind: "oauth".into(),
            gateway_token_hash: vec![],
            encrypted_credentials: None,
            credential_version: 0,
            status: "connected".into(),
            expires_at: None,
            last_error_code: None,
        };
        let credentials = StoredCredentials {
            access_token: "upstream-secret".into(),
            refresh_token: None,
            token_endpoint: None,
            client_id: None,
            client_secret: None,
            resource: format!("http://{addr}/mcp"),
            revocation_endpoint: None,
            cf_access_client_id: None,
            cf_access_client_secret: None,
        };
        let response = forward(&request, &connection, &credentials).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["mcp-session-id"], "s-2");
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body, "data: 1\n\ndata: 2\n\n");
        let captured = captured.lock().await.to_ascii_lowercase();
        assert!(captured.contains("authorization: bearer upstream-secret"));
        assert!(!captured.contains("attacker"));
        assert!(captured.contains("mcp-session-id: s-1"));
    }
}
