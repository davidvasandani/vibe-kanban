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

const AUTH_METHODS: [&str; 2] = ["xai.api_key", "cached_token"];

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

