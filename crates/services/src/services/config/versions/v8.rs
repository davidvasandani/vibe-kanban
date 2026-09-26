use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
pub use v7::{
    EditorConfig, EditorType, GitHubConfig, NotificationConfig, ShowcaseState, SoundFile,
    ThemeMode, UiLanguage,
};

use crate::services::config::versions::v7;

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

fn default_commit_reminder_enabled() -> bool {
    true
}

fn default_relay_enabled() -> bool {
    true
}

/// Opt-in auto error remediation: when a coding-agent turn fails, file an
/// issue carrying the configured pipelines and start an unattended workspace
/// on it. Off by default because it spawns agents (which may merge to the base
/// branch) without a human in the loop; see
/// `services::services::error_remediation` for the loop and rate guards.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(default)]
pub struct AutoErrorRemediationConfig {
    pub enabled: bool,
    /// Remote project that receives the issue. `None` uses the failing
    /// workspace's own linked project (and skips when it has none).
    pub project_id: Option<Uuid>,
    /// Repositories for the remediation workspace. Empty uses the failing
    /// workspace's repositories.
    pub repo_ids: Vec<Uuid>,
    pub executor: BaseCodingAgent,
    /// Executor profile variant. Falls back to the default variant when the
    /// named variant is not defined.
    pub variant: Option<String>,
    pub model_id: Option<String>,
    /// Pipelines attached to the issue, each with its default stages.
    pub pipeline_ids: Vec<String>,
    /// Stage ids that make the run merge to the base branch. When none of
    /// them is already a default stage, the first one present in the selected
    /// pipelines is enabled, so an unattended run always ends in a merge.
    pub merge_stage_ids: Vec<String>,
    /// Global cap on launches per trailing hour; 0 disables launching.
    pub max_per_hour: u32,
}

impl Default for AutoErrorRemediationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            project_id: None,
            repo_ids: Vec::new(),
            executor: BaseCodingAgent::ClaudeCode,
            variant: Some("PROALIGN".to_string()),
            model_id: Some("claude-opus-5-5".to_string()),
            pipeline_ids: vec!["wikillm".to_string(), "speckit".to_string()],
            merge_stage_ids: vec!["pr-and-merge".to_string(), "merge".to_string()],
            max_per_hour: 3,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
pub enum SendMessageShortcut {
    #[default]
    ModifierEnter,
    Enter,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Config {
    pub config_version: String,
    pub theme: ThemeMode,
    pub executor_profile: ExecutorProfileId,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    #[serde(default)]
    pub remote_onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    pub analytics_enabled: bool,
    pub workspace_dir: Option<String>,
    pub last_app_version: Option<String>,
    pub show_release_notes: bool,
    #[serde(default)]
    pub language: UiLanguage,
    #[serde(default = "default_git_branch_prefix")]
    pub git_branch_prefix: String,
    #[serde(default)]
    pub showcases: ShowcaseState,
    #[serde(default = "default_pr_auto_description_enabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(default)]
    pub pr_auto_description_prompt: Option<String>,
    #[serde(default = "default_commit_reminder_enabled")]
    pub commit_reminder_enabled: bool,
    #[serde(default)]
    pub commit_reminder_prompt: Option<String>,
    #[serde(default)]
    pub send_message_shortcut: SendMessageShortcut,
    #[serde(default = "default_relay_enabled")]
    pub relay_enabled: bool,
    #[serde(default)]
    pub host_nickname: Option<String>,
    /// Opt-in: auto-resume coding-agent runs interrupted by a server
    /// restart. Off by default because it spawns agents (and spends
    /// tokens) at boot without a human in the loop.
    #[serde(default)]
    pub resume_interrupted_on_startup: bool,
    #[serde(default)]
    pub auto_error_remediation: AutoErrorRemediationConfig,
}

impl Config {
    fn from_v7_config(old_config: v7::Config) -> Self {
        // Convert Option<bool> to bool: None or Some(true) become true, Some(false) stays false
        let analytics_enabled = old_config.analytics_enabled.unwrap_or(true);

        Self {
            config_version: "v8".to_string(),
            theme: old_config.theme,
            executor_profile: old_config.executor_profile,
            disclaimer_acknowledged: old_config.disclaimer_acknowledged,
            onboarding_acknowledged: old_config.onboarding_acknowledged,
            remote_onboarding_acknowledged: false,
            notifications: old_config.notifications,
            editor: old_config.editor,
            github: old_config.github,
            analytics_enabled,
            workspace_dir: old_config.workspace_dir,
            last_app_version: old_config.last_app_version,
            show_release_notes: old_config.show_release_notes,
            language: old_config.language,
            git_branch_prefix: old_config.git_branch_prefix,
            showcases: old_config.showcases,
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            commit_reminder_enabled: true,
            commit_reminder_prompt: None,
            send_message_shortcut: SendMessageShortcut::default(),
            relay_enabled: true,
            host_nickname: None,
            resume_interrupted_on_startup: false,
            auto_error_remediation: AutoErrorRemediationConfig::default(),
        }
    }

    pub fn from_previous_version(raw_config: &str) -> Result<Self, Error> {
        let old_config = v7::Config::from(raw_config.to_string());
        Ok(Self::from_v7_config(old_config))
    }
}

impl From<String> for Config {
    fn from(raw_config: String) -> Self {
        if let Ok(config) = serde_json::from_str::<Config>(&raw_config)
            && config.config_version == "v8"
        {
            return config;
        }

        match Self::from_previous_version(&raw_config) {
            Ok(config) => {
                tracing::info!("Config upgraded to v8");
                config
            }
            Err(e) => {
                tracing::warn!("Config migration failed: {}, using default", e);
                Self::default()
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: "v8".to_string(),
            theme: ThemeMode::System,
            executor_profile: ExecutorProfileId::new(BaseCodingAgent::ClaudeCode),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            remote_onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            analytics_enabled: true,
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            commit_reminder_enabled: true,
            commit_reminder_prompt: None,
            send_message_shortcut: SendMessageShortcut::default(),
            relay_enabled: true,
            host_nickname: None,
            resume_interrupted_on_startup: false,
            auto_error_remediation: AutoErrorRemediationConfig::default(),
        }
    }
}
