use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Who currently owns mutation rights on a browser session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserController {
    None,
    Agent {
        execution_id: Uuid,
    },
    Human {
        user_id: String,
        connection_id: Uuid,
    },
}

impl BrowserController {
    pub fn is_none(&self) -> bool {
        matches!(self, BrowserController::None)
    }

    pub fn execution_id(&self) -> Option<Uuid> {
        match self {
            BrowserController::Agent { execution_id } => Some(*execution_id),
            _ => None,
        }
    }

    pub fn connection_id(&self) -> Option<Uuid> {
        match self {
            BrowserController::Human { connection_id, .. } => Some(*connection_id),
            _ => None,
        }
    }
}

/// Identity attempting a control operation or mutating command.
/// Resolved server-side; never taken verbatim from a client payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlPrincipal {
    Agent {
        execution_id: Uuid,
    },
    Human {
        user_id: String,
        connection_id: Uuid,
    },
}

impl ControlPrincipal {
    pub fn to_controller(&self) -> BrowserController {
        match self {
            ControlPrincipal::Agent { execution_id } => BrowserController::Agent {
                execution_id: *execution_id,
            },
            ControlPrincipal::Human {
                user_id,
                connection_id,
            } => BrowserController::Human {
                user_id: user_id.clone(),
                connection_id: *connection_id,
            },
        }
    }

    pub fn matches(&self, controller: &BrowserController) -> bool {
        &self.to_controller() == controller
    }
}

/// Why a control transition happened; persisted for audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ControlTransitionReason {
    Acquire,
    Release,
    Transfer,
    Takeover,
    Expired,
    Disconnected,
    ExecutionCompleted,
    Closed,
}

impl ControlTransitionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Acquire => "acquire",
            Self::Release => "release",
            Self::Transfer => "transfer",
            Self::Takeover => "takeover",
            Self::Expired => "expired",
            Self::Disconnected => "disconnected",
            Self::ExecutionCompleted => "execution_completed",
            Self::Closed => "closed",
        }
    }
}

/// Current control state as reported to clients.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BrowserControlState {
    pub controller: BrowserController,
    #[ts(type = "number")]
    pub generation: u64,
    pub lease_expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(use_ts_enum)]
pub enum BrowserSessionStatus {
    Starting,
    Running,
    Closed,
    Failed,
}

impl BrowserSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Closed => "closed",
            Self::Failed => "failed",
        }
    }
}

/// Live, host-authoritative view of one session; broadcast to observers on
/// every change and included in list/detail API responses.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BrowserSessionLiveState {
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub status: BrowserSessionStatus,
    pub current_url: Option<String>,
    pub page_title: Option<String>,
    pub control: BrowserControlState,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// A mutating browser command. Read-only operations (screenshot, page info)
/// are deliberately not part of this enum.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserAction {
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Reload,
    Click {
        x: i32,
        y: i32,
        button: Option<MouseButton>,
    },
    /// Raw mouse move (live-view WS only; coalesced client-side).
    MouseMove {
        x: i32,
        y: i32,
    },
    Type {
        text: String,
    },
    Key {
        key: String,
        modifiers: Option<Vec<String>>,
    },
    SetViewport {
        width: u32,
        height: u32,
    },
    /// Privileged capability; gated by config.
    Evaluate {
        expression: String,
    },
}

impl BrowserAction {
    pub fn is_privileged(&self) -> bool {
        matches!(self, BrowserAction::Evaluate { .. })
    }
}

/// Result of a successfully executed action.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BrowserActionResult {
    pub command_id: Uuid,
    #[ts(type = "number")]
    pub generation: u64,
    /// Present for Evaluate.
    pub value: Option<serde_json::Value>,
    pub current_url: Option<String>,
}

/// Read-only page information.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BrowserPageInfo {
    pub url: Option<String>,
    pub title: Option<String>,
    pub console_tail: Vec<String>,
}

/// One screencast frame (JPEG bytes + dimensions).
#[derive(Debug, Clone)]
pub struct BrowserFrame {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub data: bytes::Bytes,
}

/// Typed error codes. String form is the stable wire contract used by REST
/// (ApiError payload), the live-view WS, and MCP in-band errors.
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BrowserSessionError {
    /// Another controller holds the lease; acquisition refused.
    #[error("browser session is controlled by another controller")]
    ControlConflict {
        controller: BrowserController,
        #[ts(type = "number")]
        generation: u64,
    },
    /// expected_generation did not match the current generation.
    #[error("stale browser control generation")]
    StaleGeneration {
        controller: BrowserController,
        #[ts(type = "number")]
        generation: u64,
    },
    /// The caller's control was displaced; retryable once control returns.
    #[error("browser control was lost")]
    ControlLost {
        controller: BrowserController,
        #[ts(type = "number")]
        generation: u64,
    },
    /// Agent acquire without execution_id and no running coding-agent
    /// execution in the workspace.
    #[error("no running execution to bind browser control to")]
    NoRunningExecution,
    #[error("browser session is closed")]
    SessionClosed,
    #[error("browser session not found")]
    NotFound,
    #[error("no usable browser is available: {message}")]
    BrowserUnavailable { message: String },
    #[error("browser capability denied: {capability}")]
    CapabilityDenied { capability: String },
    #[error("browser driver error: {message}")]
    #[serde(rename = "DRIVER_ERROR")]
    Driver { message: String },
    #[error("browser command timed out")]
    Timeout,
    #[error("browser session storage error: {message}")]
    #[serde(rename = "STORAGE_ERROR")]
    Storage { message: String },
    /// The named execution does not belong to the session's workspace.
    #[error("execution does not belong to this workspace")]
    ExecutionNotInWorkspace,
}

impl BrowserSessionError {
    /// Stable machine-readable code (matches the serde tag).
    pub fn code(&self) -> &'static str {
        match self {
            Self::ControlConflict { .. } => "CONTROL_CONFLICT",
            Self::StaleGeneration { .. } => "STALE_GENERATION",
            Self::ControlLost { .. } => "CONTROL_LOST",
            Self::NoRunningExecution => "NO_RUNNING_EXECUTION",
            Self::SessionClosed => "SESSION_CLOSED",
            Self::NotFound => "NOT_FOUND",
            Self::BrowserUnavailable { .. } => "BROWSER_UNAVAILABLE",
            Self::CapabilityDenied { .. } => "CAPABILITY_DENIED",
            Self::Driver { .. } => "DRIVER_ERROR",
            Self::Timeout => "TIMEOUT",
            Self::Storage { .. } => "STORAGE_ERROR",
            Self::ExecutionNotInWorkspace => "EXECUTION_NOT_IN_WORKSPACE",
        }
    }

    /// Whether an agent should treat this as a pause-and-retry-later signal.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::ControlLost { .. } | Self::ControlConflict { .. } | Self::Timeout
        )
    }
}

impl From<sqlx::Error> for BrowserSessionError {
    fn from(e: sqlx::Error) -> Self {
        BrowserSessionError::Storage {
            message: e.to_string(),
        }
    }
}
