//! On-demand connectivity + health probing for configured MCP servers.
//!
//! Vibe Kanban only ever *writes* MCP config into agent config files; it never
//! connects to the servers. This module lets the UI verify, on demand, that each
//! configured server is reachable and actually serving tools: it performs the
//! minimal MCP handshake (`initialize` -> `notifications/initialized` ->
//! `tools/list`) over the server's transport and reports the outcome.
//!
//! There is no MCP *client* elsewhere in the workspace (rmcp is used only as a
//! server, and its 1.3 client transports do not cover legacy SSE), so this is a
//! small hand-rolled JSON-RPC probe built on crates already in the workspace
//! (`reqwest`, `tokio`, `serde_json`, `eventsource-stream`). Each probe is
//! wrapped in a timeout by the caller so a hung server cannot block the request.

use std::{
    collections::HashMap,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderName, HeaderValue, WWW_AUTHENTICATE};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, Lines,
    },
    time::timeout,
};
use ts_rs::TS;

/// Protocol version advertised to servers during `initialize`. Servers negotiate
/// down/echo their own, so a recent value maximizes compatibility.
const PROTOCOL_VERSION: &str = "2025-06-18";
const INIT_ID: i64 = 1;
const TOOLS_ID: i64 = 2;

/// Outcome of testing a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpServerTestStatus {
    /// Connected, handshake completed, tools listed.
    Ok,
    /// A recognized transport that failed to connect / handshake / list tools.
    Failed,
    /// An HTTP/SSE probe was rejected with 401/403: the server is up but wants
    /// credentials Vibe Kanban doesn't have.
    AuthRequired,
    /// The config shape was not recognized as any known transport (no probe run).
    Unsupported,
}

/// Per-server result returned to the UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct McpServerTestResult {
    pub name: String,
    /// `"stdio" | "http" | "sse" | "unknown"`.
    pub transport: String,
    pub status: McpServerTestStatus,
    pub latency_ms: Option<u64>,
    pub tool_count: Option<u32>,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub error: Option<String>,
    /// Raw `WWW-Authenticate` header from a 401/403 probe response, when the
    /// server sent one (per RFC 9728 it points at the protected-resource
    /// metadata needed to start OAuth).
    pub www_authenticate: Option<String>,
}

/// Probe failure, split so auth rejections can be surfaced distinctly.
#[derive(Debug)]
enum ProbeError {
    /// HTTP 401/403 from an http/sse transport.
    AuthRequired {
        www_authenticate: Option<String>,
        message: String,
    },
    Other(String),
}

impl From<String> for ProbeError {
    fn from(message: String) -> Self {
        ProbeError::Other(message)
    }
}

/// A normalized, probe-ready view of an (untyped) MCP server config entry.
#[derive(Debug, Clone, PartialEq, Eq)]
enum McpProbeTarget {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
    Sse {
        url: String,
        headers: HashMap<String, String>,
    },
    Unsupported {
        reason: String,
    },
}

impl McpProbeTarget {
    fn transport_label(&self) -> &'static str {
        match self {
            McpProbeTarget::Stdio { .. } => "stdio",
            McpProbeTarget::Http { .. } => "http",
            McpProbeTarget::Sse { .. } => "sse",
            McpProbeTarget::Unsupported { .. } => "unknown",
        }
    }
}

/// The subset of a successful handshake we surface to the UI.
#[derive(Debug)]
struct ProbeOk {
    server_name: Option<String>,
    server_version: Option<String>,
    tool_count: Option<u32>,
}

/// Test every server concurrently, each bounded by `per_server_timeout`.
/// Results are stable-sorted by server name so repeated runs are comparable.
pub async fn test_mcp_servers(
    servers: HashMap<String, Value>,
    per_server_timeout: Duration,
) -> Vec<McpServerTestResult> {
    let client = reqwest::Client::new();
    let futures = servers.into_iter().map(|(name, value)| {
        let client = client.clone();
        async move { test_one(&client, name, &value, per_server_timeout).await }
    });
    let mut results = futures::future::join_all(futures).await;
    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

async fn test_one(
    client: &reqwest::Client,
    name: String,
    value: &Value,
    per_server_timeout: Duration,
) -> McpServerTestResult {
    let target = normalize(value);
    let transport = target.transport_label().to_string();

    if let McpProbeTarget::Unsupported { reason } = &target {
        return McpServerTestResult {
            name,
            transport,
            status: McpServerTestStatus::Unsupported,
            latency_ms: None,
            tool_count: None,
            server_name: None,
            server_version: None,
            error: Some(reason.clone()),
            www_authenticate: None,
        };
    }

    let start = Instant::now();
    let outcome = timeout(per_server_timeout, run_probe(client, &target)).await;
    match outcome {
        Ok(Ok(ok)) => McpServerTestResult {
            name,
            transport,
            status: McpServerTestStatus::Ok,
            latency_ms: Some(start.elapsed().as_millis() as u64),
            tool_count: ok.tool_count,
            server_name: ok.server_name,
            server_version: ok.server_version,
            error: None,
            www_authenticate: None,
        },
        Ok(Err(err)) => failed(name, transport, err),
        Err(_) => failed(
            name,
            transport,
            ProbeError::Other(format!(
                "timed out after {}s",
                per_server_timeout.as_secs().max(1)
            )),
        ),
    }
}

fn failed(name: String, transport: String, error: ProbeError) -> McpServerTestResult {
    let (status, message, www_authenticate) = match error {
        ProbeError::AuthRequired {
            www_authenticate,
            message,
        } => (McpServerTestStatus::AuthRequired, message, www_authenticate),
        ProbeError::Other(message) => (McpServerTestStatus::Failed, message, None),
    };
    McpServerTestResult {
        name,
        transport,
        status,
        latency_ms: None,
        tool_count: None,
        server_name: None,
        server_version: None,
        error: Some(message),
        www_authenticate,
    }
}

async fn run_probe(
    client: &reqwest::Client,
    target: &McpProbeTarget,
) -> Result<ProbeOk, ProbeError> {
    match target {
        McpProbeTarget::Stdio { command, args, env } => probe_stdio(command, args, env)
            .await
            .map_err(ProbeError::Other),
        McpProbeTarget::Http { url, headers } => probe_http(client, url, headers).await,
        McpProbeTarget::Sse { url, headers } => probe_sse(client, url, headers).await,
        McpProbeTarget::Unsupported { reason } => Err(ProbeError::Other(reason.clone())),
    }
}

// --- Transport normalization ------------------------------------------------

/// Convert an untyped server config entry into a probe target, tolerant of the
/// agent-specific shapes Vibe Kanban writes (see `mcp_config.rs` adapters).
fn normalize(value: &Value) -> McpProbeTarget {
    let Some(obj) = value.as_object() else {
        return McpProbeTarget::Unsupported {
            reason: "server config is not an object".to_string(),
        };
    };

    // A server the agent won't start (e.g. Opencode `enabled: false`) must not
    // be spawned or contacted by the probe.
    if obj.get("enabled").and_then(Value::as_bool) == Some(false) {
        return McpProbeTarget::Unsupported {
            reason: "server is disabled".to_string(),
        };
    }

    let type_str = obj.get("type").and_then(Value::as_str);

    // Opencode stdio: `type: "local"` with `command` as an array.
    if type_str == Some("local")
        && let Some(arr) = obj.get("command").and_then(Value::as_array)
    {
        let mut parts = arr.iter().filter_map(Value::as_str);
        if let Some(command) = parts.next() {
            return McpProbeTarget::Stdio {
                command: command.to_string(),
                args: parts.map(String::from).collect(),
                env: string_map(obj, &["env", "environment"]),
            };
        }
    }

    // General stdio: string `command` and not an http/sse/remote transport.
    if let Some(command) = obj.get("command").and_then(Value::as_str)
        && !matches!(type_str, Some("http") | Some("sse") | Some("remote"))
    {
        let args = obj
            .get("args")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        return McpProbeTarget::Stdio {
            command: command.to_string(),
            args,
            env: string_map(obj, &["env", "environment"]),
        };
    }

    // URL-based transports (http / streamable-http / legacy sse).
    if let Some(url) = obj
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| obj.get("httpUrl").and_then(Value::as_str))
    {
        let headers = string_map(obj, &["headers", "http_headers"]);
        if type_str == Some("sse") || url_is_sse(url) {
            return McpProbeTarget::Sse {
                url: url.to_string(),
                headers,
            };
        }
        return McpProbeTarget::Http {
            url: url.to_string(),
            headers,
        };
    }

    McpProbeTarget::Unsupported {
        reason: "unrecognized MCP server shape (no runnable command or url)".to_string(),
    }
}

fn url_is_sse(url: &str) -> bool {
    let path = match reqwest::Url::parse(url) {
        Ok(u) => u.path().trim_end_matches('/').to_string(),
        Err(_) => url
            .split('?')
            .next()
            .unwrap_or(url)
            .trim_end_matches('/')
            .to_string(),
    };
    path.ends_with("/sse")
}

fn string_map(obj: &serde_json::Map<String, Value>, keys: &[&str]) -> HashMap<String, String> {
    for key in keys {
        if let Some(Value::Object(map)) = obj.get(*key) {
            return map
                .iter()
                .filter_map(|(k, v)| value_to_string(v).map(|s| (k.clone(), s)))
                .collect();
        }
    }
    HashMap::new()
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// --- JSON-RPC message helpers -----------------------------------------------

fn initialize_request(id: i64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "vibe-kanban", "version": env!("CARGO_PKG_VERSION") }
        }
    })
}

fn initialized_notification() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

fn tools_list_request(id: i64) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list" })
}

/// Extract the `result` from a JSON-RPC response, or turn an `error` into `Err`.
fn extract_result(msg: &Value) -> Result<Value, String> {
    if let Some(err) = msg.get("error") {
        let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
        let message = err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Err(format!("server error {code}: {message}"));
    }
    Ok(msg.get("result").cloned().unwrap_or(Value::Null))
}

fn parse_server_info(result: &Value) -> (Option<String>, Option<String>) {
    let info = result.get("serverInfo");
    let name = info
        .and_then(|s| s.get("name"))
        .and_then(Value::as_str)
        .map(String::from);
    let version = info
        .and_then(|s| s.get("version"))
        .and_then(Value::as_str)
        .map(String::from);
    (name, version)
}

fn count_tools(result: &Value) -> Option<u32> {
    result
        .get("tools")
        .and_then(Value::as_array)
        .map(|t| t.len() as u32)
}

/// Find the JSON-RPC message with `id` in a body that may be a single object or
/// a batch array.
fn find_by_id(body: &Value, id: i64) -> Option<Value> {
    match body {
        Value::Array(items) => items
            .iter()
            .find(|m| m.get("id").and_then(Value::as_i64) == Some(id))
            .cloned(),
        Value::Object(_) => {
            (body.get("id").and_then(Value::as_i64) == Some(id)).then(|| body.clone())
        }
        _ => None,
    }
}

fn snippet(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 200 {
        format!("{}…", s.chars().take(200).collect::<String>())
    } else {
        s.to_string()
    }
}

// --- stdio probe ------------------------------------------------------------

async fn probe_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> Result<ProbeOk, String> {
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args)
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn `{command}`: {e}"))?;

    let stdin = child.stdin.take().ok_or("failed to open stdin")?;
    let stdout = child.stdout.take().ok_or("failed to open stdout")?;

    // Drain stderr in the background so a chatty child can't deadlock on a full
    // pipe; keep it to attach to error messages.
    let stderr_buf = Arc::new(tokio::sync::Mutex::new(String::new()));
    if let Some(stderr) = child.stderr.take() {
        let buf = stderr_buf.clone();
        tokio::spawn(async move {
            let mut collected = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut collected).await;
            *buf.lock().await = collected;
        });
    }

    let result = mcp_handshake_over_io(stdin, BufReader::new(stdout)).await;
    // Terminate and reap the child so a finished/failed probe can't leave a
    // zombie behind across repeated tests. (On the timeout path the probe
    // future is dropped and `kill_on_drop` handles reaping instead.)
    let _ = child.start_kill();
    let _ = child.wait().await;

    result.map_err(|e| {
        let stderr = stderr_buf
            .try_lock()
            .ok()
            .map(|s| snippet(&s))
            .filter(|s| !s.is_empty());
        match stderr {
            Some(s) => format!("{e} (stderr: {s})"),
            None => e,
        }
    })
}

/// The transport-agnostic newline-delimited JSON-RPC handshake used for stdio.
/// Generic over the IO so it can be unit-tested against an in-memory duplex.
async fn mcp_handshake_over_io(
    mut writer: impl AsyncWrite + Unpin,
    reader: impl AsyncBufRead + Unpin,
) -> Result<ProbeOk, String> {
    let mut lines = reader.lines();

    write_line(&mut writer, &initialize_request(INIT_ID)).await?;
    let init = read_result_for_id(&mut lines, INIT_ID).await?;
    let (server_name, server_version) = parse_server_info(&init);

    write_line(&mut writer, &initialized_notification()).await?;
    write_line(&mut writer, &tools_list_request(TOOLS_ID)).await?;
    let tools = read_result_for_id(&mut lines, TOOLS_ID).await?;

    Ok(ProbeOk {
        server_name,
        server_version,
        tool_count: count_tools(&tools),
    })
}

async fn write_line(writer: &mut (impl AsyncWrite + Unpin), msg: &Value) -> Result<(), String> {
    let mut line = serde_json::to_string(msg).map_err(|e| e.to_string())?;
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|e| format!("write error: {e}"))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("flush error: {e}"))
}

async fn read_result_for_id<R: AsyncBufRead + Unpin>(
    lines: &mut Lines<R>,
    id: i64,
) -> Result<Value, String> {
    loop {
        match lines
            .next_line()
            .await
            .map_err(|e| format!("read error: {e}"))?
        {
            Some(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(msg) = serde_json::from_str::<Value>(line) else {
                    continue; // skip log lines / non-JSON noise
                };
                if msg.get("id").and_then(Value::as_i64) == Some(id) {
                    return extract_result(&msg);
                }
            }
            None => return Err("server closed the connection before responding".to_string()),
        }
    }
}

// --- streamable HTTP probe --------------------------------------------------

/// Turn a non-success HTTP response into a `ProbeError`, classifying 401/403
/// as `AuthRequired` and capturing the `WWW-Authenticate` header.
async fn http_status_error(resp: reqwest::Response) -> ProbeError {
    let status = resp.status();
    let www_authenticate = resp
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let text = resp.text().await.unwrap_or_default();
    let message = format!("HTTP {}: {}", status.as_u16(), snippet(&text));
    if matches!(status.as_u16(), 401 | 403) {
        ProbeError::AuthRequired {
            www_authenticate,
            message,
        }
    } else {
        ProbeError::Other(message)
    }
}

async fn probe_http(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
) -> Result<ProbeOk, ProbeError> {
    let (init, session) = http_send(
        client,
        url,
        headers,
        None,
        &initialize_request(INIT_ID),
        Some(INIT_ID),
    )
    .await?;
    let init = init.unwrap_or(Value::Null);
    let (server_name, server_version) = parse_server_info(&init);

    let (_, session2) = http_send(
        client,
        url,
        headers,
        session.as_deref(),
        &initialized_notification(),
        None,
    )
    .await?;
    let session = session2.or(session);

    let (tools, _) = http_send(
        client,
        url,
        headers,
        session.as_deref(),
        &tools_list_request(TOOLS_ID),
        Some(TOOLS_ID),
    )
    .await?;

    Ok(ProbeOk {
        server_name,
        server_version,
        tool_count: tools.as_ref().and_then(count_tools),
    })
}

/// POST one JSON-RPC message. When `want_id` is set, read the response (JSON body
/// or SSE stream) and return its `result`. Returns any `Mcp-Session-Id` header.
async fn http_send(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
    session: Option<&str>,
    body: &Value,
    want_id: Option<i64>,
) -> Result<(Option<Value>, Option<String>), ProbeError> {
    let mut req = client
        .post(url)
        .header(ACCEPT, "application/json, text/event-stream")
        .header(CONTENT_TYPE, "application/json")
        .header("MCP-Protocol-Version", PROTOCOL_VERSION);
    req = apply_headers(req, headers);
    if let Some(sid) = session {
        req = req.header("Mcp-Session-Id", sid);
    }

    let body = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let resp = req
        .body(body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    let status = resp.status();
    let new_session = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    if !status.is_success() {
        return Err(http_status_error(resp).await);
    }

    let Some(want) = want_id else {
        return Ok((None, new_session)); // notification: nothing to read
    };

    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if content_type.contains("text/event-stream") {
        let mut stream = resp.bytes_stream().eventsource();
        let result = read_event_result(&mut stream, want).await?;
        Ok((Some(result), new_session))
    } else {
        let text = resp
            .text()
            .await
            .map_err(|e| format!("failed to read response: {e}"))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|e| format!("invalid JSON response: {e}"))?;
        let msg = find_by_id(&value, want)
            .ok_or_else(|| "response did not contain a matching id".to_string())?;
        Ok((Some(extract_result(&msg)?), new_session))
    }
}

// --- legacy SSE probe -------------------------------------------------------

async fn probe_sse(
    client: &reqwest::Client,
    url: &str,
    headers: &HashMap<String, String>,
) -> Result<ProbeOk, ProbeError> {
    let req = apply_headers(client.get(url).header(ACCEPT, "text/event-stream"), headers);
    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(http_status_error(resp).await);
    }

    let mut stream = resp.bytes_stream().eventsource();

    // The server announces its message-POST endpoint via an `endpoint` event.
    let endpoint = loop {
        match stream.next().await {
            Some(Ok(ev)) if ev.event == "endpoint" => break ev.data.trim().to_string(),
            Some(Ok(_)) => continue,
            Some(Err(e)) => return Err(format!("SSE stream error: {e}").into()),
            None => return Err("SSE stream closed before endpoint event".to_string().into()),
        }
    };

    let base = reqwest::Url::parse(url).map_err(|e| format!("invalid server url: {e}"))?;
    let message_url = base
        .join(&endpoint)
        .map_err(|e| format!("invalid endpoint url `{endpoint}`: {e}"))?;

    sse_post(client, &message_url, headers, &initialize_request(INIT_ID)).await?;
    let init = read_event_result(&mut stream, INIT_ID).await?;
    let (server_name, server_version) = parse_server_info(&init);

    sse_post(client, &message_url, headers, &initialized_notification()).await?;
    sse_post(client, &message_url, headers, &tools_list_request(TOOLS_ID)).await?;
    let tools = read_event_result(&mut stream, TOOLS_ID).await?;

    Ok(ProbeOk {
        server_name,
        server_version,
        tool_count: count_tools(&tools),
    })
}

async fn sse_post(
    client: &reqwest::Client,
    message_url: &reqwest::Url,
    headers: &HashMap<String, String>,
    body: &Value,
) -> Result<(), ProbeError> {
    let req = apply_headers(
        client
            .post(message_url.clone())
            .header(CONTENT_TYPE, "application/json"),
        headers,
    );
    let body = serde_json::to_string(body).map_err(|e| e.to_string())?;
    let resp = req
        .body(body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(http_status_error(resp).await);
    }
    Ok(())
}

/// Read an SSE event stream until a JSON-RPC message with `id` arrives.
async fn read_event_result<S, E>(stream: &mut S, id: i64) -> Result<Value, String>
where
    S: Stream<Item = Result<eventsource_stream::Event, E>> + Unpin,
    E: std::fmt::Display,
{
    while let Some(event) = stream.next().await {
        let event = event.map_err(|e| format!("stream error: {e}"))?;
        let data = event.data.trim();
        if data.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if msg.get("id").and_then(Value::as_i64) == Some(id) {
            return extract_result(&msg);
        }
    }
    Err("event stream closed before a matching response".to_string())
}

fn apply_headers(
    mut req: reqwest::RequestBuilder,
    headers: &HashMap<String, String>,
) -> reqwest::RequestBuilder {
    for (key, value) in headers {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(key.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            req = req.header(name, val);
        }
    }
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_stdio_command_string() {
        let v = json!({ "command": "npx", "args": ["-y", "server"], "env": { "TOKEN": "x" } });
        assert_eq!(
            normalize(&v),
            McpProbeTarget::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "server".into()],
                env: HashMap::from([("TOKEN".into(), "x".into())]),
            }
        );
    }

    #[test]
    fn normalize_opencode_local_command_array() {
        let v = json!({ "type": "local", "command": ["node", "server.js"], "enabled": true });
        assert_eq!(
            normalize(&v),
            McpProbeTarget::Stdio {
                command: "node".into(),
                args: vec!["server.js".into()],
                env: HashMap::new(),
            }
        );
    }

    #[test]
    fn normalize_http_by_type() {
        let v = json!({ "type": "http", "url": "https://example.com/mcp" });
        assert_eq!(
            normalize(&v),
            McpProbeTarget::Http {
                url: "https://example.com/mcp".into(),
                headers: HashMap::new(),
            }
        );
    }

    #[test]
    fn normalize_gemini_http_url() {
        let v = json!({ "httpUrl": "https://example.com/mcp", "headers": { "Accept": "x" } });
        assert_eq!(
            normalize(&v),
            McpProbeTarget::Http {
                url: "https://example.com/mcp".into(),
                headers: HashMap::from([("Accept".into(), "x".into())]),
            }
        );
    }

    #[test]
    fn normalize_codex_http_headers() {
        let v = json!({
            "url": "https://example.com/mcp",
            "http_headers": { "Authorization": "Bearer token" }
        });
        assert_eq!(
            normalize(&v),
            McpProbeTarget::Http {
                url: "https://example.com/mcp".into(),
                headers: HashMap::from([("Authorization".into(), "Bearer token".into())]),
            }
        );
    }

    #[test]
    fn normalize_sse_by_type_and_by_path() {
        let by_type = json!({ "type": "sse", "url": "https://example.com/x" });
        assert!(matches!(normalize(&by_type), McpProbeTarget::Sse { .. }));

        let by_path = json!({ "url": "http://127.0.0.1:3334/sse" });
        assert!(matches!(normalize(&by_path), McpProbeTarget::Sse { .. }));

        let trailing = json!({ "url": "http://127.0.0.1:3334/sse/" });
        assert!(matches!(normalize(&trailing), McpProbeTarget::Sse { .. }));
    }

    #[test]
    fn normalize_remote_url_is_http_not_sse() {
        let v = json!({ "type": "remote", "url": "https://example.com/mcp" });
        assert!(matches!(normalize(&v), McpProbeTarget::Http { .. }));
    }

    #[test]
    fn normalize_disabled_server_is_not_probed() {
        // Even with an otherwise-runnable stdio shape, `enabled: false` must not
        // produce a probe target.
        let v = json!({ "type": "local", "command": ["node", "s.js"], "enabled": false });
        match normalize(&v) {
            McpProbeTarget::Unsupported { reason } => assert!(reason.contains("disabled")),
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn normalize_unrecognized_shape() {
        assert!(matches!(
            normalize(&json!({ "foo": "bar" })),
            McpProbeTarget::Unsupported { .. }
        ));
        assert!(matches!(
            normalize(&json!("not an object")),
            McpProbeTarget::Unsupported { .. }
        ));
    }

    #[test]
    fn parse_helpers() {
        let init = json!({ "serverInfo": { "name": "mock", "version": "1.2.3" } });
        assert_eq!(
            parse_server_info(&init),
            (Some("mock".to_string()), Some("1.2.3".to_string()))
        );
        let tools = json!({ "tools": [ {}, {}, {} ] });
        assert_eq!(count_tools(&tools), Some(3));
        assert_eq!(count_tools(&json!({})), None);
    }

    #[test]
    fn extract_result_reports_errors() {
        let ok = json!({ "id": 1, "result": { "x": 1 } });
        assert_eq!(extract_result(&ok).unwrap(), json!({ "x": 1 }));
        let err = json!({ "id": 1, "error": { "code": -32601, "message": "no method" } });
        assert!(extract_result(&err).unwrap_err().contains("no method"));
    }

    #[test]
    fn find_by_id_object_and_batch() {
        let obj = json!({ "id": 2, "result": {} });
        assert!(find_by_id(&obj, 2).is_some());
        assert!(find_by_id(&obj, 9).is_none());
        let batch = json!([{ "id": 1, "result": {} }, { "id": 2, "result": { "a": 1 } }]);
        assert_eq!(
            find_by_id(&batch, 2).unwrap(),
            json!({ "id": 2, "result": { "a": 1 } })
        );
    }

    /// Drive the stdio handshake against an in-memory server that speaks the
    /// newline-delimited JSON-RPC protocol — no external process required.
    #[tokio::test]
    async fn handshake_over_duplex_reports_tools_and_info() {
        let (client_io, server_io) = tokio::io::duplex(8192);
        let (server_read, server_write) = tokio::io::split(server_io);

        tokio::spawn(async move {
            let mut lines = BufReader::new(server_read).lines();
            let mut writer = server_write;
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
                let id = msg.get("id").and_then(Value::as_i64);
                let reply = match method {
                    "initialize" => Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "serverInfo": { "name": "mock", "version": "9.9" } }
                    })),
                    "tools/list" => Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "tools": [ { "name": "a" }, { "name": "b" } ] }
                    })),
                    _ => None, // notifications get no reply
                };
                if let Some(reply) = reply {
                    let mut out = serde_json::to_string(&reply).unwrap();
                    out.push('\n');
                    if writer.write_all(out.as_bytes()).await.is_err() {
                        break;
                    }
                }
            }
        });

        let (client_read, client_write) = tokio::io::split(client_io);
        let ok = mcp_handshake_over_io(client_write, BufReader::new(client_read))
            .await
            .expect("handshake should succeed");
        assert_eq!(ok.tool_count, Some(2));
        assert_eq!(ok.server_name.as_deref(), Some("mock"));
        assert_eq!(ok.server_version.as_deref(), Some("9.9"));
    }

    #[tokio::test]
    async fn probe_stdio_bogus_command_fails() {
        let err = probe_stdio("vk-definitely-not-a-real-binary-xyz", &[], &HashMap::new())
            .await
            .unwrap_err();
        assert!(err.contains("failed to spawn"), "got: {err}");
    }

    /// Serve one canned HTTP/1.1 response on a loopback listener, returning
    /// the URL to hit. Enough for probes that fail on their first request.
    async fn one_shot_http_server(response: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf).await;
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn http_401_with_www_authenticate_is_auth_required() {
        let url = one_shot_http_server(
            "HTTP/1.1 401 Unauthorized\r\n\
             WWW-Authenticate: Bearer resource_metadata=\"https://x.dev/prm\"\r\n\
             content-length: 12\r\nconnection: close\r\n\r\nunauthorized",
        )
        .await;
        let servers = HashMap::from([("s".to_string(), json!({ "type": "http", "url": url }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(5)).await;
        assert_eq!(results[0].status, McpServerTestStatus::AuthRequired);
        assert_eq!(
            results[0].www_authenticate.as_deref(),
            Some("Bearer resource_metadata=\"https://x.dev/prm\"")
        );
        let error = results[0].error.as_deref().unwrap();
        assert!(error.contains("HTTP 401"), "got: {error}");
    }

    #[tokio::test]
    async fn http_403_is_auth_required_even_without_header() {
        let url = one_shot_http_server(
            "HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        let servers = HashMap::from([("s".to_string(), json!({ "type": "http", "url": url }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(5)).await;
        assert_eq!(results[0].status, McpServerTestStatus::AuthRequired);
        assert_eq!(results[0].www_authenticate, None);
        assert!(results[0].error.as_deref().unwrap().contains("HTTP 403"));
    }

    #[tokio::test]
    async fn http_500_is_plain_failure() {
        let url = one_shot_http_server(
            "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 4\r\nconnection: close\r\n\r\nboom",
        )
        .await;
        let servers = HashMap::from([("s".to_string(), json!({ "type": "http", "url": url }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(5)).await;
        assert_eq!(results[0].status, McpServerTestStatus::Failed);
        assert_eq!(results[0].www_authenticate, None);
        assert!(results[0].error.as_deref().unwrap().contains("HTTP 500"));
    }

    #[tokio::test]
    async fn sse_401_is_auth_required() {
        let base = one_shot_http_server(
            "HTTP/1.1 401 Unauthorized\r\n\
             WWW-Authenticate: Bearer realm=\"mcp\"\r\n\
             content-length: 0\r\nconnection: close\r\n\r\n",
        )
        .await;
        let url = format!("{base}/sse");
        let servers = HashMap::from([("s".to_string(), json!({ "url": url }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(5)).await;
        assert_eq!(results[0].transport, "sse");
        assert_eq!(results[0].status, McpServerTestStatus::AuthRequired);
        assert_eq!(
            results[0].www_authenticate.as_deref(),
            Some("Bearer realm=\"mcp\"")
        );
    }

    #[tokio::test]
    async fn connection_refused_is_plain_failure() {
        // Bind-then-drop to get a port with nothing listening.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let servers = HashMap::from([("s".to_string(), json!({ "type": "http", "url": url }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(5)).await;
        assert_eq!(results[0].status, McpServerTestStatus::Failed);
        assert_eq!(results[0].www_authenticate, None);
    }

    #[tokio::test]
    async fn test_mcp_servers_marks_unsupported() {
        let servers = HashMap::from([("weird".to_string(), json!({ "nope": true }))]);
        let results = test_mcp_servers(servers, Duration::from_secs(1)).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, McpServerTestStatus::Unsupported);
        assert_eq!(results[0].transport, "unknown");
        assert!(results[0].error.is_some());
    }
}
