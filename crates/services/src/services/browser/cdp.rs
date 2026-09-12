//! Raw Chrome-DevTools-Protocol driver.
//!
//! Spawns a locally installed Chromium with `--remote-debugging-port=0`
//! (loopback only), speaks CDP over a single WebSocket, and exposes the small
//! command surface `DriverHandle` needs. The Chromium process is spawned as
//! its own process group and killed group-wide on close — `kill_on_drop`
//! alone is not trusted to reap grandchildren (see
//! wiki/agent-process-lifecycle.md).

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use base64::Engine;
use command_group::AsyncCommandGroup;
use futures::{SinkExt, StreamExt, stream::SplitSink};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpStream,
    sync::{Mutex as AsyncMutex, broadcast, oneshot},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use super::{
    driver::{BrowserDriver, DriverError, DriverHandle, LaunchOpts},
    types::{BrowserAction, BrowserFrame, BrowserPageInfo, MouseButton},
};

const CDP_CALL_TIMEOUT: Duration = Duration::from_secs(15);
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(20);
const CONSOLE_TAIL_CAPACITY: usize = 100;

/// Locate a Chromium/Chrome executable: explicit config, env override, then
/// well-known names/paths.
pub fn discover_chromium(config_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = config_path {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(p) = std::env::var("VIBE_BROWSER_CHROMIUM_PATH") {
        let path = PathBuf::from(p);
        if path.is_file() {
            return Some(path);
        }
    }
    let names = [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "headless_shell",
        "chrome",
    ];
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in names {
                let candidate = dir.join(name);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    let fixed = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    ];
    fixed.iter().map(PathBuf::from).find(|p| p.is_file())
}

pub struct CdpDriver {
    executable: PathBuf,
}

impl CdpDriver {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }

    fn user_data_dir(profile: Option<&str>) -> PathBuf {
        match profile {
            Some(name) => {
                let sanitized: String = name
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                dirs::data_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("vibe-kanban")
                    .join("browser-profiles")
                    .join(sanitized)
            }
            None => std::env::temp_dir()
                .join("vibe-kanban-browser")
                .join(uuid::Uuid::new_v4().to_string()),
        }
    }
}

#[async_trait]
impl BrowserDriver for CdpDriver {
    async fn launch(&self, opts: LaunchOpts) -> Result<Box<dyn DriverHandle>, DriverError> {
        let user_data_dir = Self::user_data_dir(opts.profile.as_deref());
        tokio::fs::create_dir_all(&user_data_dir)
            .await
            .map_err(|e| DriverError::Protocol(format!("failed to create profile dir: {e}")))?;

        let mut command = tokio::process::Command::new(&self.executable);
        command
            .arg("--remote-debugging-port=0")
            .arg("--remote-allow-origins=*")
            .arg(format!("--user-data-dir={}", user_data_dir.display()))
            .arg("--headless=new")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-gpu")
            .arg("--hide-scrollbars")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .group()
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| DriverError::Unavailable(format!("failed to spawn chromium: {e}")))?;
        // The group leader's pid doubles as the pgid for grouped spawns.
        let pgid = child.id().map(|pid| pid as i32);

        // Chromium prints "DevTools listening on ws://..." to stderr once the
        // debugging endpoint is up.
        let stderr = child
            .inner()
            .stderr
            .take()
            .ok_or_else(|| DriverError::Protocol("chromium stderr not captured".to_string()))?;
        let mut lines = BufReader::new(stderr).lines();
        let ws_url = tokio::time::timeout(LAUNCH_TIMEOUT, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(idx) = line.find("ws://") {
                    return Some(line[idx..].trim().to_string());
                }
            }
            None
        })
        .await
        .map_err(|_| DriverError::Timeout)?
        .ok_or_else(|| {
            DriverError::Unavailable("chromium exited before exposing DevTools".to_string())
        })?;
        // Keep draining stderr so the pipe never blocks Chromium.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });

        let (ws, _) = connect_async(&ws_url)
            .await
            .map_err(|e| DriverError::Protocol(format!("CDP connect failed: {e}")))?;
        let (sink, stream) = ws.split();

        let conn = Arc::new(CdpConnection {
            next_id: AtomicI64::new(1),
            pending: Mutex::new(HashMap::new()),
            writer: AsyncMutex::new(sink),
        });
        let (frames_tx, _) = broadcast::channel(32);
        let console = Arc::new(Mutex::new(VecDeque::new()));
        let frame_seq = Arc::new(AtomicU64::new(0));

        let reader = tokio::spawn(reader_loop(
            stream,
            conn.clone(),
            frames_tx.clone(),
            console.clone(),
            frame_seq,
        ));

        // Attach to the initial page target (create one if needed).
        let targets = conn.call(None, "Target.getTargets", json!({})).await?;
        let target_id = targets["targetInfos"].as_array().and_then(|infos| {
            infos
                .iter()
                .find(|t| t["type"] == "page")
                .and_then(|t| t["targetId"].as_str())
                .map(str::to_string)
        });
        let target_id = match target_id {
            Some(id) => id,
            None => conn
                .call(None, "Target.createTarget", json!({ "url": "about:blank" }))
                .await?["targetId"]
                .as_str()
                .ok_or_else(|| DriverError::Protocol("no targetId".to_string()))?
                .to_string(),
        };
        let session_id = conn
            .call(
                None,
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
            )
            .await?["sessionId"]
            .as_str()
            .ok_or_else(|| DriverError::Protocol("no sessionId".to_string()))?
            .to_string();

        let handle = CdpHandle {
            conn,
            session_id: session_id.clone(),
            child: AsyncMutex::new(Some(child)),
            pgid,
            frames: frames_tx,
            console,
            reader,
        };
        handle.call("Page.enable", json!({})).await?;
        handle.call("Runtime.enable", json!({})).await?;
        if let Some((width, height)) = opts.viewport {
            handle
                .call(
                    "Emulation.setDeviceMetricsOverride",
                    json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": false }),
                )
                .await?;
        }
        handle
            .call(
                "Page.startScreencast",
                json!({ "format": "jpeg", "quality": 60, "maxWidth": 1280, "maxHeight": 960, "everyNthFrame": 2 }),
            )
            .await?;
        Ok(Box::new(handle))
    }
}

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

struct CdpConnection {
    next_id: AtomicI64,
    pending: Mutex<HashMap<i64, oneshot::Sender<Result<Value, String>>>>,
    writer: AsyncMutex<WsSink>,
}

impl CdpConnection {
    async fn call(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value, DriverError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            msg["sessionId"] = json!(session_id);
        }
        {
            let mut writer = self.writer.lock().await;
            writer
                .send(Message::text(msg.to_string()))
                .await
                .map_err(|e| {
                    self.pending.lock().unwrap().remove(&id);
                    DriverError::Protocol(format!("CDP send failed: {e}"))
                })?;
        }
        let response = tokio::time::timeout(CDP_CALL_TIMEOUT, rx)
            .await
            .map_err(|_| {
                self.pending.lock().unwrap().remove(&id);
                DriverError::Timeout
            })?
            .map_err(|_| DriverError::Gone)?;
        response.map_err(DriverError::Protocol)
    }

    /// Fire-and-forget send (used from the reader task for screencast acks —
    /// awaiting a response there would deadlock the reader).
    async fn send_no_wait(&self, session_id: Option<&str>, method: &str, params: Value) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            msg["sessionId"] = json!(session_id);
        }
        let mut writer = self.writer.lock().await;
        let _ = writer.send(Message::text(msg.to_string())).await;
    }
}

async fn reader_loop(
    mut stream: futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    conn: Arc<CdpConnection>,
    frames: broadcast::Sender<BrowserFrame>,
    console: Arc<Mutex<VecDeque<String>>>,
    frame_seq: Arc<AtomicU64>,
) {
    while let Some(Ok(msg)) = stream.next().await {
        let Ok(text) = msg.to_text() else { continue };
        let Ok(value) = serde_json::from_str::<Value>(text) else {
            continue;
        };
        if let Some(id) = value["id"].as_i64() {
            if let Some(tx) = conn.pending.lock().unwrap().remove(&id) {
                let result = if value["error"].is_object() {
                    Err(value["error"]["message"]
                        .as_str()
                        .unwrap_or("CDP error")
                        .to_string())
                } else {
                    Ok(value["result"].clone())
                };
                let _ = tx.send(result);
            }
            continue;
        }
        match value["method"].as_str() {
            Some("Page.screencastFrame") => {
                let params = &value["params"];
                let session = value["sessionId"].as_str().map(str::to_string);
                if let Some(data) = params["data"].as_str()
                    && let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data)
                {
                    let frame = BrowserFrame {
                        seq: frame_seq.fetch_add(1, Ordering::Relaxed) + 1,
                        width: params["metadata"]["deviceWidth"].as_u64().unwrap_or(0) as u32,
                        height: params["metadata"]["deviceHeight"].as_u64().unwrap_or(0) as u32,
                        data: bytes::Bytes::from(bytes),
                    };
                    let _ = frames.send(frame);
                }
                if let Some(ack) = params["sessionId"].as_i64() {
                    conn.send_no_wait(
                        session.as_deref(),
                        "Page.screencastFrameAck",
                        json!({ "sessionId": ack }),
                    )
                    .await;
                }
            }
            Some("Runtime.consoleAPICalled") => {
                let params = &value["params"];
                let kind = params["type"].as_str().unwrap_or("log");
                let preview: Vec<String> = params["args"]
                    .as_array()
                    .map(|args| {
                        args.iter()
                            .filter_map(|a| {
                                a.get("value")
                                    .map(|v| v.to_string())
                                    .or_else(|| a["description"].as_str().map(str::to_string))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut console = console.lock().unwrap();
                console.push_back(format!("[{kind}] {}", preview.join(" ")));
                while console.len() > CONSOLE_TAIL_CAPACITY {
                    console.pop_front();
                }
            }
            _ => {}
        }
    }
    // Socket closed: fail all pending calls so callers see Gone, not a hang.
    let mut pending = conn.pending.lock().unwrap();
    for (_, tx) in pending.drain() {
        let _ = tx.send(Err("CDP connection closed".to_string()));
    }
}

pub struct CdpHandle {
    conn: Arc<CdpConnection>,
    session_id: String,
    child: AsyncMutex<Option<command_group::AsyncGroupChild>>,
    pgid: Option<i32>,
    frames: broadcast::Sender<BrowserFrame>,
    console: Arc<Mutex<VecDeque<String>>>,
    reader: tokio::task::JoinHandle<()>,
}

impl CdpHandle {
    async fn call(&self, method: &str, params: Value) -> Result<Value, DriverError> {
        self.conn.call(Some(&self.session_id), method, params).await
    }

    async fn navigation_history(&self) -> Result<(i64, Vec<Value>), DriverError> {
        let history = self.call("Page.getNavigationHistory", json!({})).await?;
        let index = history["currentIndex"].as_i64().unwrap_or(0);
        let entries = history["entries"].as_array().cloned().unwrap_or_default();
        Ok((index, entries))
    }

    async fn navigate_history(&self, delta: i64) -> Result<(), DriverError> {
        let (index, entries) = self.navigation_history().await?;
        let target = index + delta;
        if target < 0 || target as usize >= entries.len() {
            return Ok(()); // boundary: nothing to navigate to
        }
        let entry_id = entries[target as usize]["id"]
            .as_i64()
            .ok_or_else(|| DriverError::Protocol("history entry without id".to_string()))?;
        self.call(
            "Page.navigateToHistoryEntry",
            json!({ "entryId": entry_id }),
        )
        .await?;
        Ok(())
    }
}

fn mouse_button(button: &Option<MouseButton>) -> &'static str {
    match button {
        Some(MouseButton::Middle) => "middle",
        Some(MouseButton::Right) => "right",
        _ => "left",
    }
}

#[async_trait]
impl DriverHandle for CdpHandle {
    async fn perform(&self, action: &BrowserAction) -> Result<Option<Value>, DriverError> {
        match action {
            BrowserAction::Navigate { url } => {
                let result = self.call("Page.navigate", json!({ "url": url })).await?;
                if let Some(error) = result["errorText"].as_str() {
                    return Err(DriverError::Protocol(format!("navigation failed: {error}")));
                }
                Ok(None)
            }
            BrowserAction::Back => self.navigate_history(-1).await.map(|_| None),
            BrowserAction::Forward => self.navigate_history(1).await.map(|_| None),
            BrowserAction::Reload => {
                self.call("Page.reload", json!({})).await?;
                Ok(None)
            }
            BrowserAction::Click { x, y, button } => {
                let button = mouse_button(button);
                self.call(
                    "Input.dispatchMouseEvent",
                    json!({ "type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": 1 }),
                )
                .await?;
                self.call(
                    "Input.dispatchMouseEvent",
                    json!({ "type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": 1 }),
                )
                .await?;
                Ok(None)
            }
            BrowserAction::MouseMove { x, y } => {
                self.call(
                    "Input.dispatchMouseEvent",
                    json!({ "type": "mouseMoved", "x": x, "y": y }),
                )
                .await?;
                Ok(None)
            }
            BrowserAction::Type { text } => {
                self.call("Input.insertText", json!({ "text": text }))
                    .await?;
                Ok(None)
            }
            BrowserAction::Key { key, modifiers } => {
                let modifier_mask = modifiers
                    .as_ref()
                    .map(|mods| {
                        mods.iter()
                            .map(|m| match m.to_ascii_lowercase().as_str() {
                                "alt" => 1,
                                "ctrl" | "control" => 2,
                                "meta" | "cmd" => 4,
                                "shift" => 8,
                                _ => 0,
                            })
                            .fold(0, |acc, m| acc | m)
                    })
                    .unwrap_or(0);
                // Enter needs a carriage-return text payload to submit forms.
                let text = match key.as_str() {
                    "Enter" => Some("\r"),
                    k if k.chars().count() == 1 => Some(key.as_str()),
                    _ => None,
                };
                let mut down = json!({ "type": "keyDown", "key": key, "modifiers": modifier_mask });
                if let Some(text) = text {
                    down["text"] = json!(text);
                }
                self.call("Input.dispatchKeyEvent", down).await?;
                self.call(
                    "Input.dispatchKeyEvent",
                    json!({ "type": "keyUp", "key": key, "modifiers": modifier_mask }),
                )
                .await?;
                Ok(None)
            }
            BrowserAction::SetViewport { width, height } => {
                self.call(
                    "Emulation.setDeviceMetricsOverride",
                    json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": false }),
                )
                .await?;
                Ok(None)
            }
            BrowserAction::Evaluate { expression } => {
                let result = self
                    .call(
                        "Runtime.evaluate",
                        json!({ "expression": expression, "returnByValue": true, "awaitPromise": true, "timeout": 10_000 }),
                    )
                    .await?;
                if result["exceptionDetails"].is_object() {
                    let text = result["exceptionDetails"]["text"]
                        .as_str()
                        .unwrap_or("evaluation failed");
                    return Err(DriverError::Protocol(text.to_string()));
                }
                Ok(Some(result["result"]["value"].clone()))
            }
        }
    }

    async fn page_info(&self) -> Result<BrowserPageInfo, DriverError> {
        let (index, entries) = self.navigation_history().await?;
        let current = entries.get(index as usize);
        Ok(BrowserPageInfo {
            url: current.and_then(|e| e["url"].as_str()).map(str::to_string),
            title: current
                .and_then(|e| e["title"].as_str())
                .map(str::to_string),
            console_tail: self.console.lock().unwrap().iter().cloned().collect(),
        })
    }

    async fn screenshot(&self) -> Result<Vec<u8>, DriverError> {
        let result = self
            .call("Page.captureScreenshot", json!({ "format": "png" }))
            .await?;
        let data = result["data"]
            .as_str()
            .ok_or_else(|| DriverError::Protocol("screenshot missing data".to_string()))?;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| DriverError::Protocol(format!("screenshot decode failed: {e}")))
    }

    fn subscribe_frames(&self) -> broadcast::Receiver<BrowserFrame> {
        self.frames.subscribe()
    }

    fn pgid(&self) -> Option<i32> {
        self.pgid
    }

    async fn close(&self) {
        // Polite shutdown first, then group kill — never rely on
        // kill_on_drop to reap Chromium's helper processes.
        let _ = tokio::time::timeout(
            Duration::from_secs(2),
            self.conn.call(None, "Browser.close", json!({})),
        )
        .await;
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = utils::process::kill_process_group(&mut child).await;
        }
        self.reader.abort();
    }
}
