pub mod auth_refresh;
pub mod client;
pub mod jsonrpc;
pub mod normalize_logs;
pub mod review;
pub mod rollout_transfer;
pub mod slash_commands;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

/// Returns the Codex home directory.
///
/// Checks the `CODEX_HOME` environment variable first, then falls back to `~/.codex`.
/// This allows users to configure a custom location for Codex configuration and state.
pub fn codex_home() -> Option<PathBuf> {
    if let Ok(codex_home) = env::var("CODEX_HOME")
        && !codex_home.trim().is_empty()
    {
        return Some(PathBuf::from(codex_home));
    }
    dirs::home_dir().map(|home| home.join(".codex"))
}

#[cfg(test)]
mod tests {
    use super::Codex;

    #[test]
    fn deployment_codex_command_is_fail_closed_and_blank_values_fall_back() {
        assert_eq!(
            Codex::base_command_from_deployment(Some(
                "/nix/store/test-vibe-kanban-codex/bin/codex".to_string()
            )),
            (
                "/nix/store/test-vibe-kanban-codex/bin/codex".to_string(),
                true
            )
        );
        assert_eq!(
            Codex::base_command_from_deployment(Some("  ".to_string())),
            (Codex::base_command().to_string(), false)
        );
    }

    #[test]
    fn app_server_always_uses_strict_config() {
        let codex: Codex = serde_json::from_value(serde_json::json!({}))
            .expect("default Codex config deserializes");
        let builder = codex.build_command_builder().expect("valid Codex command");

        assert_eq!(
            builder.params,
            Some(vec![
                "app-server".to_string(),
                "--strict-config".to_string()
            ]),
            "the pinned app-server must reject unknown configuration fields"
        );
    }

    /// Pins the *exact* config key that disables Codex's persistent-PTY
    /// background execution.
    ///
    /// This is not redundant with reading the code: upstream's `ConfigToml` has
    /// no `serde(deny_unknown_fields)` and VK does not pass `--strict-config`,
    /// so an unknown key is silently ignored. A misspelling would leave a
    /// control that is present in review, present on the wire, and completely
    /// inert. The literal is therefore written out here rather than referenced
    /// through a constant.
    #[test]
    fn unified_exec_feature_key_is_pinned_and_disabled() {
        let codex: Codex =
            serde_json::from_value(serde_json::json!({})).expect("default Codex config");
        let params = codex.build_thread_start_params(std::path::Path::new("/tmp"));
        let config = params.config.expect("config overrides present");

        assert_eq!(
            config.get("features.unified_exec"),
            Some(&serde_json::Value::Bool(false)),
            "exact key `features.unified_exec` must be set to false; got {config:?}"
        );
        assert!(
            !config.contains_key("include_apply_patch_tool"),
            "removed V1 field must never be emitted as an inert config key"
        );

        // `features.shell_tool` would disable ordinary execution entirely and
        // must never be touched here.
        assert!(
            !config.keys().any(|key| key.contains("shell_tool")),
            "shell_tool must not be configured; got {config:?}"
        );
    }

    /// The key is unconditional — it must not depend on approval policy,
    /// sandbox, or any other user-facing setting.
    #[test]
    fn unified_exec_disabled_regardless_of_other_settings() {
        for settings in [
            serde_json::json!({}),
            serde_json::json!({"sandbox": "danger-full-access"}),
            serde_json::json!({"ask_for_approval": "never"}),
            serde_json::json!({"ask_for_approval": "on-request", "profile": "custom"}),
        ] {
            let codex: Codex =
                serde_json::from_value(settings.clone()).expect("Codex config deserializes");
            let params = codex.build_thread_start_params(std::path::Path::new("/tmp"));
            let config = params.config.expect("config overrides present");
            assert_eq!(
                config.get("features.unified_exec"),
                Some(&serde_json::Value::Bool(false)),
                "settings {settings} must still disable unified_exec"
            );
        }
    }
}

pub(crate) fn resolve_model(model: Option<&str>) -> (Option<&str>, bool) {
    match model.and_then(|m| m.strip_suffix("-fast")) {
        Some(base) => (Some(base), true),
        None => (model, false),
    }
}

pub(crate) fn fork_params_from(thread_id: String, params: ThreadStartParams) -> ThreadForkParams {
    ThreadForkParams {
        thread_id,
        model: params.model,
        model_provider: params.model_provider,
        cwd: params.cwd,
        approval_policy: params.approval_policy,
        sandbox: params.sandbox,
        config: params.config,
        base_instructions: params.base_instructions,
        developer_instructions: params.developer_instructions,
        service_tier: params.service_tier,
        ..Default::default()
    }
}

use async_trait::async_trait;
use codex_app_server_protocol::{
    AskForApproval as V2AskForApproval, ReviewTarget, SandboxMode as V2SandboxMode,
    ThreadForkParams, ThreadStartParams, UserInput,
};
use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum_macros::{AsRefStr, EnumString};
use tokio::process::Command;
use ts_rs::TS;
use workspace_utils::{command_ext::GroupSpawnNoWindowExt, msg_store::MsgStore};

use self::{
    client::{AppServerClient, LogWriter},
    jsonrpc::{ExitSignalSender, JsonRpcPeer},
    normalize_logs::{Error, normalize_logs},
};
use crate::{
    approvals::ExecutorApprovalService,
    command::{CmdOverrides, CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::ExecutorDiscoveredOptions,
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, ExecutorExitResult,
        SlashCommandDescription, SpawnedChild, StandardCodingAgentExecutor,
    },
    logs::utils::patch,
    mcp_refresh::McpRefreshHandle,
    model_selector::{ModelInfo, ModelSelectorConfig, PermissionPolicy, ReasoningOption},
    profile::ExecutorConfig,
    stdout_dup::create_stdout_pipe_writer,
};

/// Sandbox policy modes for Codex
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SandboxMode {
    Auto,
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

/// Determines when the user is consulted to approve Codex actions.
///
/// - `UnlessTrusted`: Read-only commands are auto-approved. Everything else will
///   ask the user to approve.
/// - `OnFailure`: All commands run in a restricted sandbox initially. If a
///   command fails, the user is asked to approve execution without the sandbox.
/// - `OnRequest`: The model decides when to ask the user for approval.
/// - `Never`: Commands never ask for approval. Commands that fail in the
///   restricted sandbox are not retried.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum AskForApproval {
    UnlessTrusted,
    OnFailure,
    OnRequest,
    Never,
}

/// Reasoning effort for the underlying model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr, EnumString)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

/// Model reasoning summary style
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

/// Format for model reasoning summaries
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema, AsRefStr)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum ReasoningSummaryFormat {
    None,
    Experimental,
}

enum CodexSessionAction {
    Chat { prompt: String },
    Review { target: ReviewTarget },
}

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct Codex {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_for_approval: Option<AskForApproval>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary: Option<ReasoningSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary_format: Option<ReasoningSummaryFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(default)]
    pub plan: bool,
    #[serde(flatten)]
    pub cmd: CmdOverrides,

    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    approvals: Option<Arc<dyn ExecutorApprovalService>>,
}

#[async_trait]
impl StandardCodingAgentExecutor for Codex {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(reasoning_id) = &executor_config.reasoning_id
            && let Ok(reasoning_effort) = ReasoningEffort::from_str(reasoning_id)
        {
            self.model_reasoning_effort = Some(reasoning_effort)
        }
        if let Some(permission_policy) = &executor_config.permission_policy {
            match permission_policy {
                crate::model_selector::PermissionPolicy::Auto => {
                    self.ask_for_approval = Some(AskForApproval::Never);
                    self.plan = false;
                }
                crate::model_selector::PermissionPolicy::Supervised => {
                    if matches!(self.ask_for_approval, None | Some(AskForApproval::Never)) {
                        self.ask_for_approval = Some(AskForApproval::UnlessTrusted);
                    }
                    self.plan = false;
                }
                crate::model_selector::PermissionPolicy::Plan => {
                    self.plan = true;
                }
            }
        }
    }

    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        self.approvals = Some(approvals);
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        self.spawn_slash_command(current_dir, prompt, None, env)
            .await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        self.spawn_slash_command(current_dir, prompt, Some(session_id), env)
            .await
    }

    fn normalize_logs(
        &self,
        msg_store: Arc<MsgStore>,
        worktree_path: &Path,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        normalize_logs(msg_store, worktree_path)
    }

    fn default_mcp_config_path(&self) -> Option<PathBuf> {
        codex_home().map(|home| home.join("config.toml"))
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if let Some(timestamp) = codex_home()
            .and_then(|home| std::fs::metadata(home.join("auth.json")).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        {
            return AvailabilityInfo::LoginDetected {
                last_auth_timestamp: timestamp,
            };
        }

        let mcp_config_found = self
            .default_mcp_config_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        let installation_indicator_found = codex_home()
            .map(|home| home.join("version.json").exists())
            .unwrap_or(false);

        if mcp_config_found || installation_indicator_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        use crate::model_selector::*;
        let permission_policy = if self.plan {
            PermissionPolicy::Plan
        } else if matches!(self.ask_for_approval, None | Some(AskForApproval::Never)) {
            PermissionPolicy::Auto
        } else {
            PermissionPolicy::Supervised
        };

        ExecutorConfig {
            executor: BaseCodingAgent::Codex,
            variant: None,
            model_id: self.model.clone(),
            agent_id: None,
            reasoning_id: self
                .model_reasoning_effort
                .as_ref()
                .map(|e| e.as_ref().to_string()),
            permission_policy: Some(permission_policy),
        }
    }

    async fn discover_options(
        &self,
        _workdir: Option<&std::path::Path>,
        _repo_path: Option<&std::path::Path>,
    ) -> Result<futures::stream::BoxStream<'static, json_patch::Patch>, ExecutorError> {
        let xhigh_reasoning_options = ReasoningOption::from_names(
            [
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::Xhigh,
            ]
            .map(|e| e.as_ref().to_string()),
        );
        let max_reasoning_options = ReasoningOption::from_names(
            [
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::Xhigh,
                ReasoningEffort::Max,
            ]
            .map(|e| e.as_ref().to_string()),
        );
        let ultra_reasoning_options = ReasoningOption::from_names(
            [
                ReasoningEffort::Low,
                ReasoningEffort::Medium,
                ReasoningEffort::High,
                ReasoningEffort::Xhigh,
                ReasoningEffort::Max,
                ReasoningEffort::Ultra,
            ]
            .map(|e| e.as_ref().to_string()),
        );

        let options = ExecutorDiscoveredOptions {
            model_selector: ModelSelectorConfig {
                models: vec![
                    ModelInfo {
                        id: "gpt-5.6-sol".to_string(),
                        name: "GPT-5.6 Sol".to_string(),
                        provider_id: None,
                        reasoning_options: ultra_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.6-sol-fast".to_string(),
                        name: "GPT-5.6 Sol Fast".to_string(),
                        provider_id: None,
                        reasoning_options: ultra_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.6-terra".to_string(),
                        name: "GPT-5.6 Terra".to_string(),
                        provider_id: None,
                        reasoning_options: ultra_reasoning_options,
                    },
                    ModelInfo {
                        id: "gpt-5.6-luna".to_string(),
                        name: "GPT-5.6 Luna".to_string(),
                        provider_id: None,
                        reasoning_options: max_reasoning_options,
                    },
                    ModelInfo {
                        id: "gpt-5.4".to_string(),
                        name: "GPT-5.4".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.4-fast".to_string(),
                        name: "GPT-5.4 Fast".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.3-codex".to_string(),
                        name: "GPT-5.3 Codex".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.2-codex".to_string(),
                        name: "GPT-5.2 Codex".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.2".to_string(),
                        name: "GPT-5.2".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options.clone(),
                    },
                    ModelInfo {
                        id: "gpt-5.1-codex-max".to_string(),
                        name: "GPT-5.1 Codex Max".to_string(),
                        provider_id: None,
                        reasoning_options: xhigh_reasoning_options,
                    },
                ],
                permissions: vec![
                    PermissionPolicy::Auto,
                    PermissionPolicy::Supervised,
                    PermissionPolicy::Plan,
                ],
                ..Default::default()
            },
            slash_commands: vec![
                SlashCommandDescription {
                    name: "compact".to_string(),
                    description: Some(
                        "summarize conversation to prevent hitting the context limit".to_string(),
                    ),
                },
                SlashCommandDescription {
                    name: "init".to_string(),
                    description: Some(
                        "create an AGENTS.md file with instructions for Codex".to_string(),
                    ),
                },
                SlashCommandDescription {
                    name: "status".to_string(),
                    description: Some(
                        "show current session configuration and token usage".to_string(),
                    ),
                },
                SlashCommandDescription {
                    name: "mcp".to_string(),
                    description: Some("list configured MCP tools".to_string()),
                },
                SlashCommandDescription {
                    name: "fast".to_string(),
                    description: Some(
                        "toggle fast mode for highest speed inference (2× plan usage). Use `/fast on` or `/fast off` to set explicitly".to_string(),
                    ),
                },
            ],
            ..Default::default()
        };
        Ok(Box::pin(futures::stream::once(async move {
            patch::executor_discovered_options(options)
        })))
    }

    async fn spawn_review(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let command_parts = self.build_command_builder()?.build_initial()?;
        let review_target = ReviewTarget::Custom {
            instructions: prompt.to_string(),
        };
        let action = CodexSessionAction::Review {
            target: review_target,
        };
        self.spawn_inner(current_dir, command_parts, action, session_id, env)
            .await
    }
}

impl Codex {
    pub fn base_command() -> &'static str {
        "npx -y @openai/codex@0.144.1"
    }

    fn configured_base_command() -> (String, bool) {
        Self::base_command_from_deployment(env::var("VIBE_CODEX_COMMAND").ok())
    }

    fn base_command_from_deployment(deployment_command: Option<String>) -> (String, bool) {
        deployment_command
            .filter(|command| !command.trim().is_empty())
            .map_or_else(
                || (Self::base_command().to_string(), false),
                |command| (command, true),
            )
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        let (base_command, deployment_managed) = Self::configured_base_command();
        let mut builder = CommandBuilder::new(base_command);
        // Pinned Codex 0.144.1 accepts this app-server flag and propagates it
        // through thread config loading. Unknown config must fail visibly rather
        // than leave a reviewed control silently inert (Constitution IX).
        builder = builder.extend_params(["app-server", "--strict-config"]);
        if self.oss.unwrap_or(false) {
            builder = builder.extend_params(["--oss"]);
        }

        let mut overrides = self.cmd.clone();
        if deployment_managed {
            // A supervised deployment may require a security-patched Codex
            // build. A settings-level base command must not bypass it; other
            // parameters and environment overrides remain supported.
            overrides.base_command_override = None;
        }
        apply_overrides(builder, &overrides)
    }

    fn build_thread_start_params(&self, cwd: &Path) -> ThreadStartParams {
        let sandbox = match self.sandbox.as_ref() {
            None | Some(SandboxMode::Auto) => Some(V2SandboxMode::WorkspaceWrite), // match the Auto preset in codex
            Some(SandboxMode::ReadOnly) => Some(V2SandboxMode::ReadOnly),
            Some(SandboxMode::WorkspaceWrite) => Some(V2SandboxMode::WorkspaceWrite),
            Some(SandboxMode::DangerFullAccess) => Some(V2SandboxMode::DangerFullAccess),
        };

        let approval_policy = match self.ask_for_approval.as_ref() {
            None if matches!(self.sandbox.as_ref(), None | Some(SandboxMode::Auto)) => {
                // match the Auto preset in codex
                Some(V2AskForApproval::OnRequest)
            }
            None => None,
            Some(AskForApproval::UnlessTrusted) => Some(V2AskForApproval::UnlessTrusted),
            // v2 dropped OnFailure; upstream aliases "on-failure" to OnRequest
            Some(AskForApproval::OnFailure) => Some(V2AskForApproval::OnRequest),
            Some(AskForApproval::OnRequest) => Some(V2AskForApproval::OnRequest),
            Some(AskForApproval::Never) => Some(V2AskForApproval::Never),
        };

        let mut config = self.build_config_overrides();
        // V1 top-level params that moved into config overrides in v2
        if let Some(profile) = &self.profile {
            config
                .get_or_insert_with(HashMap::new)
                .insert("profile".to_string(), Value::String(profile.clone()));
        }
        if let Some(compact) = &self.compact_prompt {
            config
                .get_or_insert_with(HashMap::new)
                .insert("compact_prompt".to_string(), Value::String(compact.clone()));
        }
        // Close Codex's in-turn background path. `features.unified_exec`
        // (default **true** on Linux/macOS) swaps the one-shot `shell_command`
        // tool for the persistent-PTY `exec_command` / `write_stdin` pair:
        // `exec_command` returns a `session_id` while the process is still
        // running and `write_stdin` with empty `chars` polls it without
        // writing. That is a hand-rolled in-turn poller, and a VK turn is one
        // OS process group that is reaped at turn end
        // (`wiki/agent-process-lifecycle.md`), so it cannot outlive the turn
        // that created it. Disabling the feature falls back to `shell_command`;
        // the supported way to keep something running is the `spawn_poller`
        // MCP tool.
        //
        // `features.shell_tool` is deliberately left alone — turning that off
        // would disable ordinary command execution.
        //
        // Verified against the upstream source Cargo vendored for the
        // `codex-app-server-protocol` git pin `rust-v0.144.1` (see
        // `crates/executors/Cargo.toml`): `ThreadStartParams` /
        // `TurnStartParams` expose no tools allow/deny field, so this `config`
        // map is the only lever. It **fails open** — `ConfigToml` has no
        // `serde(deny_unknown_fields)` and VK does not pass `--strict-config`,
        // so a misspelled key is silently ignored and the control would look
        // present while doing nothing. The exact spelling is pinned by
        // `unified_exec_feature_key_is_pinned_and_disabled`.
        config
            .get_or_insert_with(HashMap::new)
            .insert("features.unified_exec".to_string(), Value::Bool(false));
        if !matches!(approval_policy, None | Some(V2AskForApproval::Never)) {
            let map = config.get_or_insert_with(HashMap::new);
            map.insert(
                "features.default_mode_request_user_input".to_string(),
                Value::Bool(true),
            );
            map.insert(
                "suppress_unstable_features_warning".to_string(),
                Value::Bool(true),
            );
        }

        let (model, is_fast) = resolve_model(self.model.as_deref());
        let service_tier = if is_fast {
            Some(Some("fast".to_string()))
        } else {
            None
        };

        ThreadStartParams {
            model: model.map(|m| m.to_string()),
            cwd: Some(cwd.to_string_lossy().to_string()),
            approval_policy,
            sandbox,
            config,
            base_instructions: self.base_instructions.clone(),
            model_provider: self.model_provider.clone(),
            developer_instructions: self.developer_instructions.clone(),
            service_tier,
            ..Default::default()
        }
    }

    fn build_config_overrides(&self) -> Option<HashMap<String, Value>> {
        let mut overrides = HashMap::new();

        if let Some(effort) = &self.model_reasoning_effort {
            overrides.insert(
                "model_reasoning_effort".to_string(),
                Value::String(effort.as_ref().to_string()),
            );
        }

        if let Some(summary) = &self.model_reasoning_summary {
            overrides.insert(
                "model_reasoning_summary".to_string(),
                Value::String(summary.as_ref().to_string()),
            );
        }

        if let Some(format) = &self.model_reasoning_summary_format
            && format != &ReasoningSummaryFormat::None
        {
            overrides.insert(
                "model_reasoning_summary_format".to_string(),
                Value::String(format.as_ref().to_string()),
            );
        }

        if overrides.is_empty() {
            None
        } else {
            Some(overrides)
        }
    }

    async fn spawn_inner(
        &self,
        current_dir: &Path,
        command_parts: CommandParts,
        action: CodexSessionAction,
        resume_session: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let params = self.build_thread_start_params(current_dir);
        let resume_session = resume_session.map(|s| s.to_string());

        self.spawn_app_server(
            current_dir,
            command_parts,
            env,
            move |client, _| async move {
                match action {
                    CodexSessionAction::Chat { prompt } => {
                        Self::launch_codex_agent(params, resume_session, prompt, client).await
                    }
                    CodexSessionAction::Review { target } => {
                        review::launch_codex_review(params, resume_session, target, client).await
                    }
                }
            },
        )
        .await
    }

    async fn launch_codex_agent(
        thread_start_params: ThreadStartParams,
        resume_session: Option<String>,
        combined_prompt: String,
        client: Arc<AppServerClient>,
    ) -> Result<(), ExecutorError> {
        // Serialize an up-front credential refresh across concurrent Codex
        // processes so a near-expired ChatGPT token is rotated exactly once,
        // before the turn, instead of racing (VAS-490). No-op for healthy
        // tokens and non-ChatGPT auth.
        if let Some(home) = codex_home() {
            auth_refresh::refresh_credentials_if_stale(&home, &client).await;
        }

        let account = client.get_account(false).await?;
        if account.requires_openai_auth && account.account.is_none() {
            return Err(ExecutorError::AuthRequired(
                "Codex authentication required".to_string(),
            ));
        }

        let (thread_id, resolved_model) = match resume_session {
            None => {
                let response = client.thread_start(thread_start_params).await?;
                (response.thread.id, response.model)
            }
            Some(session_id) => {
                let response = client
                    .thread_fork(fork_params_from(session_id, thread_start_params))
                    .await?;
                tracing::debug!("forked thread, new thread_id={}", response.thread.id);
                (response.thread.id, response.model)
            }
        };

        client.set_resolved_model(resolved_model);
        client.register_session(&thread_id).await?;
        let collaboration_mode = client.initial_collaboration_mode()?;
        client
            .turn_start_with_mode(
                thread_id,
                vec![UserInput::Text {
                    text: combined_prompt,
                    text_elements: vec![],
                }],
                Some(collaboration_mode),
            )
            .await?;

        Ok(())
    }

    /// Common boilerplate for spawning a Codex app server process
    /// Handles process spawning, stdout/stderr piping, exit signal handling, client initialization, and error logging.
    /// Delegates the actual Codex session logic to the provided `task` closure.
    async fn spawn_app_server<F, Fut>(
        &self,
        current_dir: &Path,
        command_parts: CommandParts,
        env: &ExecutionEnv,
        task: F,
    ) -> Result<SpawnedChild, ExecutorError>
    where
        F: FnOnce(Arc<AppServerClient>, ExitSignalSender) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), ExecutorError>> + Send + 'static,
    {
        let (program_path, args) = command_parts.into_resolved().await?;

        let mut process = Command::new(program_path);
        process
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(current_dir)
            .env("NPM_CONFIG_LOGLEVEL", "error")
            .env("NODE_NO_WARNINGS", "1")
            .env("NO_COLOR", "1")
            .env("RUST_LOG", "error")
            .args(&args);

        env.clone()
            .with_profile(&self.cmd)
            .apply_to_command(&mut process);

        let mut child = process.group_spawn_no_window()?;

        let child_stdout = child.inner().stdout.take().ok_or_else(|| {
            ExecutorError::Io(std::io::Error::other("Codex app server missing stdout"))
        })?;
        let child_stdin = child.inner().stdin.take().ok_or_else(|| {
            ExecutorError::Io(std::io::Error::other("Codex app server missing stdin"))
        })?;

        let new_stdout = create_stdout_pipe_writer(&mut child)?;
        let (exit_signal_tx, exit_signal_rx) = tokio::sync::oneshot::channel();
        let (mcp_refresh_tx, mcp_refresh_rx) = tokio::sync::oneshot::channel();
        let cancel = tokio_util::sync::CancellationToken::new();

        let auto_approve = matches!(
            (&self.sandbox, &self.ask_for_approval),
            (Some(SandboxMode::DangerFullAccess), None)
        );
        let plan_mode = self.plan;
        let approvals = self.approvals.clone();
        let repo_context = env.repo_context.clone();
        let commit_reminder = env.commit_reminder;
        let commit_reminder_prompt = env.commit_reminder_prompt.clone();
        let cancel_for_task = cancel.clone();

        tokio::spawn(async move {
            let exit_signal_tx = ExitSignalSender::new(exit_signal_tx);
            let log_writer = LogWriter::new(new_stdout);

            // Initialize the AppServerClient
            let client = AppServerClient::new(
                log_writer.clone(),
                approvals,
                auto_approve,
                plan_mode,
                repo_context,
                commit_reminder,
                commit_reminder_prompt,
                cancel_for_task.clone(),
            );
            let rpc_peer = JsonRpcPeer::spawn(
                child_stdin,
                child_stdout,
                client.clone(),
                exit_signal_tx.clone(),
                cancel_for_task,
            );
            client.connect(rpc_peer);

            let result = async {
                client.initialize().await?;
                task(client.clone(), exit_signal_tx.clone()).await?;
                // Publish the control only after thread/turn startup, so a
                // pending refresh cannot be confirmed against the pre-turn
                // global inventory.
                let _ = mcp_refresh_tx.send(McpRefreshHandle(client));
                Ok(())
            }
            .await;

            if let Err(err) = result {
                match &err {
                    ExecutorError::Io(io_err)
                        if io_err.kind() == std::io::ErrorKind::BrokenPipe =>
                    {
                        // Broken pipe likely means the parent process exited, so we can ignore it
                        return;
                    }
                    ExecutorError::AuthRequired(message) => {
                        log_writer
                            .log_raw(&Error::auth_required(message.clone()).raw())
                            .await
                            .ok();
                        exit_signal_tx
                            .send_exit_signal(ExecutorExitResult::Failure)
                            .await;
                        return;
                    }
                    _ => {
                        tracing::error!("Codex spawn error: {}", err);
                        log_writer
                            .log_raw(&Error::launch_error(err.to_string()).raw())
                            .await
                            .ok();
                    }
                }
                exit_signal_tx
                    .send_exit_signal(ExecutorExitResult::Failure)
                    .await;
            }
        });

        Ok(SpawnedChild {
            child,
            exit_signal: Some(exit_signal_rx),
            cancel: Some(cancel),
            // Phase 1: substrate only — not yet kept warm across turns. Codex
            // warm reuse requires decoupling `turn/completed` from the reader-loop
            // teardown — see specs/vk/826e-coding-agent-war/research.md (Phase 3).
            keep_warm: false,
            warm_reuse: None,
            mcp_refresh: Some(mcp_refresh_rx),
        })
    }
}
