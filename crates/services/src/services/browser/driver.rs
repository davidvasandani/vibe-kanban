use std::{collections::VecDeque, sync::Mutex, time::Duration};

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::types::{BrowserAction, BrowserFrame, BrowserPageInfo};

/// Per-operation ceiling; navigation waits and evaluation are additionally
/// bounded by the driver implementation.
pub const DRIVER_OP_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("no usable browser found: {0}")]
    Unavailable(String),
    #[error("browser process exited")]
    Gone,
    #[error("driver operation timed out")]
    Timeout,
    #[error("{0}")]
    Protocol(String),
}

#[derive(Debug, Clone, Default)]
pub struct LaunchOpts {
    /// Named profile → stable user-data-dir; None = ephemeral profile.
    pub profile: Option<String>,
    pub viewport: Option<(u32, u32)>,
}

/// Launches browser instances. One implementation drives real Chromium over
/// CDP; the mock records commands for tests/dev.
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    async fn launch(&self, opts: LaunchOpts) -> Result<Box<dyn DriverHandle>, DriverError>;
}

/// A live browser instance. All mutating entry points funnel through
/// `perform` so the session gateway has a single choke point; read-only
/// operations are separate and safe to call concurrently.
#[async_trait]
pub trait DriverHandle: Send + Sync {
    async fn perform(
        &self,
        action: &BrowserAction,
    ) -> Result<Option<serde_json::Value>, DriverError>;
    async fn page_info(&self) -> Result<BrowserPageInfo, DriverError>;
    async fn screenshot(&self) -> Result<Vec<u8>, DriverError>;
    /// Screencast frames; every observer subscribes to the same stream.
    fn subscribe_frames(&self) -> broadcast::Receiver<BrowserFrame>;
    async fn close(&self);
}

/// Test/dev driver: no real browser. Records performed actions, keeps a
/// synthetic URL/console state, emits one synthetic frame per mutation so
/// observer plumbing can be exercised.
#[derive(Default)]
pub struct MockDriver;

#[async_trait]
impl BrowserDriver for MockDriver {
    async fn launch(&self, _opts: LaunchOpts) -> Result<Box<dyn DriverHandle>, DriverError> {
        Ok(Box::new(MockHandle::new()))
    }
}

pub struct MockHandle {
    state: Mutex<MockState>,
    frames: broadcast::Sender<BrowserFrame>,
}

#[derive(Default)]
struct MockState {
    performed: Vec<BrowserAction>,
    url: Option<String>,
    history: VecDeque<String>,
    console: VecDeque<String>,
    frame_seq: u64,
    closed: bool,
}

impl MockHandle {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(MockState::default()),
            frames: broadcast::channel(16).0,
        }
    }

    pub fn performed_actions(&self) -> Vec<BrowserAction> {
        self.state.lock().unwrap().performed.clone()
    }

    pub fn is_closed(&self) -> bool {
        self.state.lock().unwrap().closed
    }
}

impl Default for MockHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DriverHandle for MockHandle {
    async fn perform(
        &self,
        action: &BrowserAction,
    ) -> Result<Option<serde_json::Value>, DriverError> {
        let mut state = self.state.lock().unwrap();
        if state.closed {
            return Err(DriverError::Gone);
        }
        state.performed.push(action.clone());
        let result = match action {
            BrowserAction::Navigate { url } => {
                if let Some(prev) = state.url.take() {
                    state.history.push_back(prev);
                }
                state.url = Some(url.clone());
                None
            }
            BrowserAction::Back => {
                if let Some(prev) = state.history.pop_back() {
                    state.url = Some(prev);
                }
                None
            }
            BrowserAction::Evaluate { expression } => {
                Some(serde_json::json!({ "evaluated": expression }))
            }
            _ => None,
        };
        state.frame_seq += 1;
        let frame = BrowserFrame {
            seq: state.frame_seq,
            width: 1280,
            height: 720,
            data: bytes::Bytes::from_static(b"mock-frame"),
        };
        drop(state);
        let _ = self.frames.send(frame);
        Ok(result)
    }

    async fn page_info(&self) -> Result<BrowserPageInfo, DriverError> {
        let state = self.state.lock().unwrap();
        if state.closed {
            return Err(DriverError::Gone);
        }
        Ok(BrowserPageInfo {
            url: state.url.clone(),
            title: state.url.as_ref().map(|u| format!("Mock: {u}")),
            console_tail: state.console.iter().cloned().collect(),
        })
    }

    async fn screenshot(&self) -> Result<Vec<u8>, DriverError> {
        if self.state.lock().unwrap().closed {
            return Err(DriverError::Gone);
        }
        Ok(b"mock-screenshot".to_vec())
    }

    fn subscribe_frames(&self) -> broadcast::Receiver<BrowserFrame> {
        self.frames.subscribe()
    }

    async fn close(&self) {
        self.state.lock().unwrap().closed = true;
    }
}

/// A driver that always fails to launch; installed when no Chromium binary
/// is discoverable so session creation degrades to a typed error.
pub struct UnavailableDriver {
    pub message: String,
}

#[async_trait]
impl BrowserDriver for UnavailableDriver {
    async fn launch(&self, _opts: LaunchOpts) -> Result<Box<dyn DriverHandle>, DriverError> {
        Err(DriverError::Unavailable(self.message.clone()))
    }
}
