use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use workspace_utils::msg_store::MsgStore;

use crate::{
    approvals::ExecutorApprovalService,
    command::{CmdOverrides, CommandBuildError, CommandBuilder, apply_overrides},
    env::ExecutionEnv,
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor, acp::AcpAgentHarness,
    },
    model_selector::{ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

// ---------------------------------------------------------------------------
// Decision record: Grok has no VK-hosted in-turn poller to close.
//
// Claude (`claude.rs`) and Codex (`codex.rs`) each carry a rule disabling their
// harness-native background/polling execution, because a VK turn is one OS
// process group reaped at turn end (`wiki/agent-process-lifecycle.md`) and
// anything backgrounded inside it dies with the turn. Grok deliberately has no
// equivalent rule, and that absence is a finding rather than an oversight:
//
//   * ACP does define terminal methods — `terminal/create`, `terminal/output`,
//     `terminal/release`, `terminal/wait_for_exit`, `terminal/kill` — which
//     together would be exactly such a poller.
//   * They are gated on the *client* advertising the capability.
//     `ClientCapabilities.terminal` is a `bool` defaulting to `false` via
//     `#[derive(Default)]`; `InitializeRequest::new()` builds its capabilities
//     with `ClientCapabilities::default()`; and `acp/harness.rs` never mutates
//     them — it reads the initialize response only for `auth_methods`. A
//     repo-wide grep for `ClientCapabilities` finds no VK call site at all.
//   * `acp/client.rs` additionally stubs all five `terminal/*` methods as
//     `Err(acp::Error::method_not_found())`, so even an agent that ignored the
//     capability flag would get nothing.
//
// There is therefore no VK-hosted terminal for Grok to poll from and nothing to
// replace with `spawn_poller`. A rule here would be a control with no mechanism
// behind it. ACP also offers no per-tool allow/deny surface —
// `NewSessionRequest` is `cwd`/`mcp_servers`/`meta`, `PromptRequest` is
// `session_id`/`prompt`/`meta` — so there is nowhere to put one.
//
// Residual, deliberately *not* guessed (Constitution IX): Grok may still
// background work through its own in-process shell tool. That tool's name could
// not be confirmed against any Grok binary, fixture, or transcript available
// here, and VK is near-blind to it regardless (ACP tool calls are classified by
// `ToolKind`, never by name). Naming a string on a hunch would ship an inert
// control that looks real; tracked as a residual risk instead.
//
// Verified against `agent-client-protocol` 0.8.0 /
// `agent-client-protocol-schema` 0.9.1; see
// `specs/vk/869c-vk-background-po/research.md`.
// ---------------------------------------------------------------------------

const AUTH_METHODS: [&str; 2] = ["cached_token", "xai.api_key"];
const AUTO_MODE: &str = "auto";
const ASK_MODE: &str = "ask";

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct Grok {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yolo: Option<bool>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    pub approvals: Option<Arc<dyn ExecutorApprovalService>>,
}

impl Grok {
    fn acp_mode(&self) -> &'static str {
        if self.yolo.unwrap_or(false) {
            AUTO_MODE
        } else {
            ASK_MODE
        }
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        let mut builder = CommandBuilder::new("grok").extend_params(["--no-auto-update"]);
        if let Some(model) = &self.model {
            builder = builder.extend_params(["--model", model]);
        }
        if self.yolo.unwrap_or(false) {
            builder = builder.extend_params(["--always-approve"]);
        }
        builder = builder.extend_params(["agent", "stdio"]);
        apply_overrides(builder, &self.cmd)
    }

    fn harness(&self) -> AcpAgentHarness {
        AcpAgentHarness::with_session_namespace("grok_sessions")
            .with_mode(self.acp_mode())
            .with_auth_methods(AUTH_METHODS)
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for Grok {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(policy) = &executor_config.permission_policy {
            self.yolo = Some(matches!(policy, PermissionPolicy::Auto));
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
        let approvals = (!self.yolo.unwrap_or(false))
            .then(|| self.approvals.clone())
            .flatten();
        self.harness()
            .spawn_with_command(
                current_dir,
                self.append_prompt.combine_prompt(prompt),
                self.build_command_builder()?.build_initial()?,
                env,
                &self.cmd,
                approvals,
            )
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
        let approvals = (!self.yolo.unwrap_or(false))
            .then(|| self.approvals.clone())
            .flatten();
        self.harness()
            .spawn_follow_up_with_command(
                current_dir,
                self.append_prompt.combine_prompt(prompt),
                session_id,
                self.build_command_builder()?.build_follow_up(&[])?,
                env,
                &self.cmd,
                approvals,
            )
            .await
    }

    fn normalize_logs(
        &self,
        msg_store: Arc<MsgStore>,
        worktree_path: &Path,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        crate::executors::acp::normalize_logs(msg_store, worktree_path)
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|home| home.join(".grok").join("config.toml"))
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        let Some(home) = dirs::home_dir() else {
            return AvailabilityInfo::NotFound;
        };
        if let Ok(metadata) = std::fs::metadata(home.join(".grok").join("auth.json"))
            && let Ok(modified) = metadata.modified()
            && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
        {
            return AvailabilityInfo::LoginDetected {
                last_auth_timestamp: duration.as_secs() as i64,
            };
        }
        if home.join(".grok").join("config.toml").exists()
            || home.join(".grok").join("bin").join("grok").exists()
        {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        ExecutorConfig {
            executor: BaseCodingAgent::Grok,
            variant: None,
            model_id: self.model.clone(),
            agent_id: None,
            reasoning_id: None,
            permission_policy: Some(if self.yolo.unwrap_or(false) {
                PermissionPolicy::Auto
            } else {
                PermissionPolicy::Supervised
            }),
        }
    }

    async fn discover_options(
        &self,
        _workdir: Option<&Path>,
        _repo_path: Option<&Path>,
    ) -> Result<futures::stream::BoxStream<'static, json_patch::Patch>, ExecutorError> {
        let options = crate::executor_discovery::ExecutorDiscoveredOptions {
            model_selector: ModelSelectorConfig {
                permissions: vec![PermissionPolicy::Auto, PermissionPolicy::Supervised],
                ..Default::default()
            },
            ..Default::default()
        };
        Ok(Box::pin(futures::stream::once(async move {
            crate::logs::utils::patch::executor_discovered_options(options)
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grok(model: Option<&str>, yolo: bool) -> Grok {
        Grok {
            append_prompt: AppendPrompt::default(),
            model: model.map(str::to_string),
            yolo: Some(yolo),
            cmd: CmdOverrides::default(),
            approvals: None,
        }
    }

    #[test]
    fn command_uses_official_acp_mode_without_updates() {
        let builder = grok(Some("grok-4.5"), true)
            .build_command_builder()
            .unwrap();
        assert_eq!(builder.base, "grok");
        assert_eq!(
            builder.params.unwrap(),
            [
                "--no-auto-update",
                "--model",
                "grok-4.5",
                "--always-approve",
                "agent",
                "stdio"
            ]
        );
    }

    #[test]
    fn supervised_mode_does_not_force_approval() {
        let builder = grok(None, false).build_command_builder().unwrap();
        assert_eq!(
            builder.params.unwrap(),
            ["--no-auto-update", "agent", "stdio"]
        );
    }

    #[test]
    fn auto_permission_uses_auto_acp_mode() {
        assert_eq!(grok(None, true).acp_mode(), AUTO_MODE);
    }

    #[test]
    fn supervised_permission_uses_ask_acp_mode() {
        assert_eq!(grok(None, false).acp_mode(), ASK_MODE);
    }

    #[test]
    fn unset_permission_uses_ask_acp_mode() {
        let mut grok = grok(None, false);
        grok.yolo = None;
        assert_eq!(grok.acp_mode(), ASK_MODE);
    }

    #[test]
    fn serialized_profile_does_not_include_runtime_approvals() {
        let value = serde_json::to_value(grok(Some("grok-4.5"), false)).unwrap();
        assert_eq!(value["model"], "grok-4.5");
        assert!(value.get("approvals").is_none());
    }
}
