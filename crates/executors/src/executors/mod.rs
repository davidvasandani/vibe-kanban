use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use command_group::AsyncGroupChild;
use enum_dispatch::enum_dispatch;
use futures::stream::BoxStream;
use futures_io::Error as FuturesIoError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::Type;
use strum_macros::{Display, EnumDiscriminants, EnumString, VariantNames};
use thiserror::Error;
use tokio::task::JoinHandle;
use ts_rs::TS;
use workspace_utils::msg_store::MsgStore;

#[cfg(feature = "qa-mode")]
use crate::executors::qa_mock::QaMockExecutor;
use crate::{
    actions::{ExecutorAction, review::RepoReviewContext},
    approvals::ExecutorApprovalService,
    command::CommandBuildError,
    env::ExecutionEnv,
    executors::{
        amp::Amp, claude::ClaudeCode, codex::Codex, copilot::Copilot, cursor::CursorAgent,
        droid::Droid, gemini::Gemini, grok::Grok, opencode::Opencode, qwen::QwenCode,
    },
    logs::utils::patch,
    mcp_config::McpConfig,
    profile::ExecutorConfig,
};

pub mod acp;
pub mod amp;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod droid;
pub mod gemini;
pub mod grok;
pub mod opencode;
#[cfg(feature = "qa-mode")]
pub mod qa_mock;
pub mod qwen;
pub mod utils;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
pub struct SlashCommandDescription {
    /// Command name without the leading slash, e.g. `help` for `/help`.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(use_ts_enum)]
pub enum BaseAgentCapability {
    SessionFork,
    /// Agent requires a setup script before it can run (e.g., login, installation)
    SetupHelper,
    /// Agent reports context/token usage information
    ContextUsage,
}

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Follow-up is not supported: {0}")]
    FollowUpNotSupported(String),
    #[error(transparent)]
    SpawnError(#[from] FuturesIoError),
    #[error("Unknown executor type: {0}")]
    UnknownExecutorType(String),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    TomlSerialize(#[from] toml::ser::Error),
    #[error(transparent)]
    TomlDeserialize(#[from] toml::de::Error),
    #[error(transparent)]
    ExecutorApprovalError(#[from] crate::approvals::ExecutorApprovalError),
    #[error(transparent)]
    CommandBuild(#[from] CommandBuildError),
    #[error("Executable `{program}` not found in PATH")]
    ExecutableNotFound { program: String },
    #[error("Setup helper not supported")]
    SetupHelperNotSupported,
    #[error("Auth required: {0}")]
    AuthRequired(String),
}

#[enum_dispatch]
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, TS, Display, EnumDiscriminants, VariantNames,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
#[strum_discriminants(
    name(BaseCodingAgent),
    // Only add Hash; Eq/PartialEq are already provided by EnumDiscriminants.
    derive(EnumString, Hash, strum_macros::Display, Serialize, Deserialize, TS, Type),
    strum(serialize_all = "SCREAMING_SNAKE_CASE"),
    ts(use_ts_enum),
    serde(rename_all = "SCREAMING_SNAKE_CASE"),
    sqlx(type_name = "TEXT", rename_all = "SCREAMING_SNAKE_CASE")
)]
pub enum CodingAgent {
    ClaudeCode,
    Amp,
    Gemini,
    Codex,
    Opencode,
    #[serde(alias = "CURSOR")]
    #[strum_discriminants(serde(alias = "CURSOR"))]
    #[strum_discriminants(strum(serialize = "CURSOR", serialize = "CURSOR_AGENT"))]
    CursorAgent,
    QwenCode,
    Copilot,
    Droid,
    Grok,
    #[cfg(feature = "qa-mode")]
    QaMock(QaMockExecutor),
}

impl CodingAgent {
    pub fn get_mcp_config(&self) -> McpConfig {
        match self {
            Self::Codex(_) => McpConfig::new(
                vec!["mcp_servers".to_string()],
                serde_json::json!({
                    "mcp_servers": {}
                }),
                self.preconfigured_mcp(),
                true,
            ),
            Self::Amp(_) => McpConfig::new(
                vec!["amp.mcpServers".to_string()],
                serde_json::json!({
                    "amp.mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
            Self::Opencode(_) => McpConfig::new(
                vec!["mcp".to_string()],
                serde_json::json!({
                    "mcp": {},
                    "$schema": "https://opencode.ai/config.json"
                }),
                self.preconfigured_mcp(),
                false,
            ),
            Self::Droid(_) => McpConfig::new(
                vec!["mcpServers".to_string()],
                serde_json::json!({
                    "mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
            Self::Grok(_) => McpConfig::new(
                vec!["mcp_servers".to_string()],
                serde_json::json!({
                    "mcp_servers": {}
                }),
                self.preconfigured_mcp(),
                true,
            ),
            _ => McpConfig::new(
                vec!["mcpServers".to_string()],
                serde_json::json!({
                    "mcpServers": {}
                }),
                self.preconfigured_mcp(),
                false,
            ),
        }
    }

    pub fn supports_mcp(&self) -> bool {
        self.default_mcp_config_path().is_some()
    }

    pub fn capabilities(&self) -> Vec<BaseAgentCapability> {
        match self {
            Self::ClaudeCode(_) => vec![
                BaseAgentCapability::SessionFork,
                BaseAgentCapability::ContextUsage,
            ],
            Self::Opencode(_) => vec![
                BaseAgentCapability::SessionFork,
                BaseAgentCapability::ContextUsage,
            ],
            Self::Codex(_) => vec![
                BaseAgentCapability::SessionFork,
                BaseAgentCapability::SetupHelper,
                BaseAgentCapability::ContextUsage,
            ],
            Self::Gemini(_) | Self::QwenCode(_) | Self::Grok(_) => {
                vec![BaseAgentCapability::SessionFork]
            }
            Self::CursorAgent(_) => vec![BaseAgentCapability::SetupHelper],
            Self::Amp(_) | Self::Copilot(_) | Self::Droid(_) => vec![],
            #[cfg(feature = "qa-mode")]
            Self::QaMock(_) => vec![], // QA mock doesn't need special capabilities
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AvailabilityInfo {
    LoginDetected { last_auth_timestamp: i64 },
    InstallationFound,
    NotFound,
}

impl AvailabilityInfo {
    pub fn is_available(&self) -> bool {
        matches!(
            self,
            AvailabilityInfo::LoginDetected { .. } | AvailabilityInfo::InstallationFound
        )
    }
}

#[async_trait]
#[enum_dispatch(CodingAgent)]
pub trait StandardCodingAgentExecutor {
    fn apply_overrides(&mut self, _executor_config: &ExecutorConfig) {}

    fn use_approvals(&mut self, _approvals: Arc<dyn ExecutorApprovalService>) {}

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError>;

    /// Continue a session, optionally resetting to a specific message.
    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError>;

    async fn spawn_review(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        match session_id {
            Some(id) => {
                self.spawn_follow_up(current_dir, prompt, id, None, env)
                    .await
            }
            None => self.spawn(current_dir, prompt, env).await,
        }
    }

    fn normalize_logs(
        &self,
        _raw_logs_event_store: Arc<MsgStore>,
        _worktree_path: &Path,
    ) -> Vec<JoinHandle<()>> {
        vec![]
    }

    // MCP configuration methods
    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf>;

    async fn get_setup_helper_action(&self) -> Result<ExecutorAction, ExecutorError> {
        Err(ExecutorError::SetupHelperNotSupported)
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        let config_files_found = self
            .default_mcp_config_path()
            .map(|path| path.exists())
            .unwrap_or(false);

        if config_files_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    /// Returns a stream of executor discovered options updates.
    async fn discover_options(
        &self,
        _workdir: Option<&Path>,
        _repo_path: Option<&Path>,
    ) -> Result<BoxStream<'static, json_patch::Patch>, ExecutorError> {
        let options = crate::executor_discovery::ExecutorDiscoveredOptions::default();
        Ok(Box::pin(futures::stream::once(async move {
            patch::executor_discovered_options(options)
        })))
    }

    /// Returns the default overrides defined by this preset/variant.
    fn get_preset_options(&self) -> ExecutorConfig;
}

/// Result communicated through the exit signal
#[derive(Debug, Clone, Copy)]
pub enum ExecutorExitResult {
    /// Process completed successfully (exit code 0)
    Success,
    /// Process should be marked as failed (non-zero exit)
    Failure,
}

/// Optional exit notification from an executor.
/// When this receiver resolves, the container should gracefully stop the process
/// and mark it according to the result.
pub type ExecutorExitSignal = tokio::sync::oneshot::Receiver<ExecutorExitResult>;

/// Cancellation token for requesting graceful shutdown of an executor.
/// When cancelled, the executor should attempt to cancel gracefully before being killed.
pub type CancellationToken = tokio_util::sync::CancellationToken;

/// Executor → Container: the connection facts a follow-up turn needs to reach a
/// warm (kept-alive) app-server instead of spawning a fresh process.
///
/// Surfaced over a `oneshot` on `SpawnedChild::warm_reuse` because some of these
/// facts (e.g. OpenCode's `base_url`) are only discovered *asynchronously* after
/// `spawn` returns — see `specs/vk/826e-coding-agent-war/research.md`. Fields are
/// transport-agnostic strings so Codex/ACP can fill a subset later (Phase 3).
#[derive(Clone)]
pub struct WarmReuseHandle {
    /// Base URL of the persistent app-server (OpenCode: `http://127.0.0.1:PORT`).
    pub base_url: String,
    /// Auth secret for the app-server. Treated as a credential: redacted from
    /// `Debug`, never logged (Constitution — VK fork "credentials never logged").
    pub server_password: String,
    /// The agent session id to continue on the warm process, when known.
    pub agent_session_id: Option<String>,
}

impl std::fmt::Debug for WarmReuseHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarmReuseHandle")
            .field("base_url", &self.base_url)
            .field("server_password", &"<redacted>")
            .field("agent_session_id", &self.agent_session_id)
            .finish()
    }
}

/// Channel by which an executor surfaces its `WarmReuseHandle` once the warm
/// process is ready (post async startup). One-shot per warm turn.
pub type WarmReuseSignal = tokio::sync::oneshot::Receiver<WarmReuseHandle>;

#[derive(Debug)]
pub struct SpawnedChild {
    pub child: AsyncGroupChild,
    /// Executor → Container: signals when executor wants to exit
    pub exit_signal: Option<ExecutorExitSignal>,
    /// Container → Executor: signals when container wants to cancel the execution
    pub cancel: Option<CancellationToken>,
    /// Executor → Container: when true, this child is a persistent app-server
    /// (Codex/OpenCode/ACP) that ends a turn by signalling over the wire rather
    /// than exiting. On a *clean* turn-completion signal the container ends the
    /// turn but keeps the process alive (warm) for the next turn instead of
    /// killing its process group. Defaults to `false`, so one-shot CLI executors
    /// that exit naturally are unaffected. See
    /// `specs/vk/1a64-coding-agent-pro/` (VAS-107 fix #2).
    pub keep_warm: bool,
    /// Executor → Container: when `keep_warm` leaves this child alive, the
    /// container awaits this receiver to learn how to drive the next turn against
    /// the warm process (stored in the container's warm registry). `None` for
    /// one-shot executors and until an executor opts in. See
    /// `specs/vk/826e-coding-agent-war/` (Phase 2).
    pub warm_reuse: Option<WarmReuseSignal>,
    /// Executor → Container: a secret-free control surface for refreshing the
    /// live executor's MCP inventory. `None` for executors without a confirmed
    /// in-session reload contract.
    pub mcp_refresh: Option<crate::mcp_refresh::McpRefreshSignal>,
}

impl From<AsyncGroupChild> for SpawnedChild {
    fn from(child: AsyncGroupChild) -> Self {
        Self {
            child,
            exit_signal: None,
            cancel: None,
            keep_warm: false,
            warm_reuse: None,
            mcp_refresh: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(transparent)]
#[schemars(
    title = "Append Prompt",
    description = "Extra text appended to the prompt",
    extend("format" = "textarea")
)]
#[derive(Default)]
pub struct AppendPrompt(pub Option<String>);

impl AppendPrompt {
    pub fn get(&self) -> Option<String> {
        self.0.clone()
    }

    pub fn combine_prompt(&self, prompt: &str) -> String {
        match self {
            AppendPrompt(Some(value)) => format!("{prompt}{value}"),
            AppendPrompt(None) => prompt.to_string(),
        }
    }
}

pub fn build_review_prompt(
    context: Option<&[RepoReviewContext]>,
    additional_prompt: Option<&str>,
) -> String {
    let mut prompt = String::from("Please review the code changes.\n\n");

    if let Some(repos) = context {
        for repo in repos {
            prompt.push_str(&format!("Repository: {}\n", repo.repo_name));
            prompt.push_str(&format!(
                "Review all changes from base commit {} to HEAD.\n",
                repo.base_commit
            ));
            prompt.push_str(&format!(
                "Use `git diff {}..HEAD` to see the changes.\n",
                repo.base_commit
            ));
            prompt.push('\n');
        }
    }

    if let Some(additional) = additional_prompt {
        prompt.push_str(additional);
    }

    prompt
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_cursor_agent_deserialization() {
        // Test that CURSOR_AGENT is accepted
        let result = BaseCodingAgent::from_str("CURSOR_AGENT");
        assert!(result.is_ok(), "CURSOR_AGENT should be valid");
        assert_eq!(result.unwrap(), BaseCodingAgent::CursorAgent);

        // Test that legacy CURSOR is still accepted for backwards compatibility
        let result = BaseCodingAgent::from_str("CURSOR");
        assert!(
            result.is_ok(),
            "CURSOR should be valid for backwards compatibility"
        );
        assert_eq!(result.unwrap(), BaseCodingAgent::CursorAgent);

        // Test serde deserialization for CURSOR_AGENT
        let result: Result<BaseCodingAgent, _> = serde_json::from_str(r#""CURSOR_AGENT""#);
        assert!(result.is_ok(), "CURSOR_AGENT should deserialize via serde");
        assert_eq!(result.unwrap(), BaseCodingAgent::CursorAgent);

        // Test serde deserialization for legacy CURSOR
        let result: Result<BaseCodingAgent, _> = serde_json::from_str(r#""CURSOR""#);
        assert!(result.is_ok(), "CURSOR should deserialize via serde");
        assert_eq!(result.unwrap(), BaseCodingAgent::CursorAgent);
    }

    #[test]
    fn warm_reuse_handle_debug_redacts_password() {
        // The reuse handle carries an app-server credential; it must never leak
        // via Debug/logs (Constitution — VK fork "credentials never logged").
        let handle = WarmReuseHandle {
            base_url: "http://127.0.0.1:4096".to_string(),
            server_password: "hunter2-super-secret".to_string(),
            agent_session_id: Some("sess-123".to_string()),
        };
        let dbg = format!("{handle:?}");
        assert!(
            !dbg.contains("hunter2-super-secret"),
            "password must be redacted"
        );
        assert!(dbg.contains("<redacted>"));
        assert!(dbg.contains("http://127.0.0.1:4096"));
        assert!(dbg.contains("sess-123"));
    }
}
