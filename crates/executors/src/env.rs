use std::{collections::HashMap, path::PathBuf};

use git::GitService;
use tokio::process::Command;

use crate::{
    command::CmdOverrides,
    executors::{CodingAgent, ExecutorError, StandardCodingAgentExecutor},
    profile::{ExecutorConfig, ExecutorConfigs, ExecutorProfile},
};

/// Repository context for executor operations
#[derive(Debug, Clone, Default)]
pub struct RepoContext {
    pub workspace_root: PathBuf,
    /// Names of repositories in the workspace (subdirectory names)
    pub repo_names: Vec<String>,
}

impl RepoContext {
    pub fn new(workspace_root: PathBuf, repo_names: Vec<String>) -> Self {
        Self {
            workspace_root,
            repo_names,
        }
    }

    pub fn repo_paths(&self) -> Vec<PathBuf> {
        self.repo_names
            .iter()
            .map(|name| self.workspace_root.join(name))
            .collect()
    }

    /// Check all repos for uncommitted changes.
    /// Returns a formatted string describing any uncommitted changes found,
    /// or an empty string if all repos are clean.
    pub async fn check_uncommitted_changes(&self) -> String {
        let repo_paths = self.repo_paths();
        if repo_paths.is_empty() {
            return String::new();
        }

        tokio::task::spawn_blocking(move || {
            let git = GitService::new();
            let mut all_status = String::new();

            for repo_path in &repo_paths {
                // Skip if not a git repository
                if !repo_path.join(".git").exists() {
                    continue;
                }

                match git.get_worktree_status(repo_path) {
                    Ok(status) if !status.entries.is_empty() => {
                        let mut status_output = String::new();
                        for entry in &status.entries {
                            status_output.push(entry.staged);
                            status_output.push(entry.unstaged);
                            status_output.push(' ');
                            status_output.push_str(&String::from_utf8_lossy(&entry.path));
                            status_output.push('\n');
                        }
                        all_status.push_str(&format!(
                            "\n{}:\n{}",
                            repo_path.display(),
                            status_output
                        ));
                    }
                    _ => {}
                }
            }

            all_status
        })
        .await
        .unwrap_or_default()
    }
}

/// Environment variables to inject into executor processes
#[derive(Debug, Clone)]
pub struct ExecutionEnv {
    pub vars: HashMap<String, String>,
    pub repo_context: RepoContext,
    pub commit_reminder: bool,
    pub commit_reminder_prompt: String,
    /// The profile definition that came *with* the request, when the process
    /// resolving it is not the one that owns `profiles.json`.
    ///
    /// A variant is user-defined data living in the coordinator's
    /// `profiles.json`; a worker has only the embedded defaults, which define
    /// `DEFAULT` and nothing else. Since scheduling treats a worker advertising
    /// a bare executor as able to run *any* variant of it, the definition has to
    /// travel with the dispatch or the worker cannot resolve what it was sent.
    /// `None` means "resolve locally", which is what every non-clustered caller
    /// wants and what a worker falls back to.
    pub executor_profile: Option<ExecutorProfile>,
}

impl ExecutionEnv {
    pub fn new(
        repo_context: RepoContext,
        commit_reminder: bool,
        commit_reminder_prompt: String,
    ) -> Self {
        Self {
            vars: HashMap::new(),
            repo_context,
            commit_reminder,
            commit_reminder_prompt,
            executor_profile: None,
        }
    }

    /// Carry a profile definition with the request, for a process that does not
    /// own `profiles.json`.
    pub fn with_executor_profile(mut self, profile: Option<ExecutorProfile>) -> Self {
        self.executor_profile = profile;
        self
    }

    /// The coding agent `config` names, preferring a definition carried with the
    /// request over this node's own profiles.
    ///
    /// The order is deliberate. A dispatched definition is what the coordinator
    /// resolved when it accepted the request, so honouring it first is what
    /// makes a worker run the same agent the user picked rather than a
    /// same-named local one. Falling back to the local cache keeps every
    /// single-node caller on exactly its previous behaviour, and keeps a worker
    /// working when the field is absent — an older coordinator, or any dispatch
    /// predating this field.
    pub fn resolve_coding_agent(
        &self,
        config: &ExecutorConfig,
    ) -> Result<CodingAgent, ExecutorError> {
        let profile_id = config.profile_id();
        // Matches `ExecutorConfigs::get_coding_agent`: an absent variant means
        // DEFAULT. Resolving the two sources differently would make the same
        // request mean different things on a coordinator and a worker.
        let variant = profile_id.variant.as_deref().unwrap_or("DEFAULT");
        let mut agent = self
            .executor_profile
            .as_ref()
            .and_then(|profile| profile.get_variant(variant))
            .cloned()
            .or_else(|| ExecutorConfigs::get_cached().get_coding_agent(&profile_id))
            .ok_or_else(|| ExecutorError::UnknownExecutorType(profile_id.to_string()))?;

        if config.has_overrides() {
            agent.apply_overrides(config);
        }
        Ok(agent)
    }

    /// Insert an environment variable
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Merge additional vars into this env. Incoming keys overwrite existing ones.
    pub fn merge(&mut self, other: &HashMap<String, String>) {
        self.vars
            .extend(other.iter().map(|(k, v)| (k.clone(), v.clone())));
    }

    /// Return a new env with overrides applied. Overrides take precedence.
    pub fn with_overrides(mut self, overrides: &HashMap<String, String>) -> Self {
        self.merge(overrides);
        self
    }

    /// Return a new env with profile env from CmdOverrides merged in.
    pub fn with_profile(self, cmd: &CmdOverrides) -> Self {
        if let Some(ref profile_env) = cmd.env {
            self.with_overrides(profile_env)
        } else {
            self
        }
    }

    /// Apply all environment variables to a Command
    pub fn apply_to_command(&self, command: &mut Command) {
        for (key, value) in &self.vars {
            command.env(key, value);
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.vars.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_overrides_runtime_env() {
        let mut base = ExecutionEnv::new(RepoContext::default(), false, String::new());
        base.insert("VK_PROJECT_NAME", "runtime");
        base.insert("FOO", "runtime");

        let mut profile = HashMap::new();
        profile.insert("FOO".to_string(), "profile".to_string());
        profile.insert("BAR".to_string(), "profile".to_string());

        let merged = base.with_overrides(&profile);

        assert_eq!(merged.vars.get("VK_PROJECT_NAME").unwrap(), "runtime");
        assert_eq!(merged.vars.get("FOO").unwrap(), "profile"); // overrides
        assert_eq!(merged.vars.get("BAR").unwrap(), "profile");
    }

    use crate::executors::BaseCodingAgent;

    /// A variant name no `default_profiles.json` defines, so a node without the
    /// operator's `profiles.json` cannot resolve it locally — which is exactly a
    /// worker's situation.
    const USER_DEFINED_VARIANT: &str = "SWEETGREEN";

    fn env_with(profile: Option<ExecutorProfile>) -> ExecutionEnv {
        ExecutionEnv::new(RepoContext::default(), false, String::new())
            .with_executor_profile(profile)
    }

    fn config(variant: Option<&str>) -> ExecutorConfig {
        ExecutorConfig {
            variant: variant.map(str::to_string),
            ..ExecutorConfig::new(BaseCodingAgent::ClaudeCode)
        }
    }

    fn dispatched(variant: &str) -> ExecutorProfile {
        let agent = ExecutorConfigs::get_cached()
            .get_coding_agent(&config(None).profile_id())
            .expect("the embedded defaults always define DEFAULT");
        ExecutorProfile {
            recently_used_models: None,
            configurations: HashMap::from([(variant.to_string(), agent)]),
        }
    }

    /// The reported bug: a turn on a worker died with
    /// `Unknown executor type: CLAUDE_CODE:SWEETGREEN`, because the variant is
    /// user-defined data that lived only on the coordinator while scheduling
    /// already treated the worker as able to run any variant of an executor it
    /// advertised.
    #[test]
    fn a_dispatched_definition_resolves_a_variant_this_node_does_not_have() {
        let requested = config(Some(USER_DEFINED_VARIANT));

        assert!(
            env_with(None).resolve_coding_agent(&requested).is_err(),
            "precondition: this node cannot resolve the variant on its own, \
             so the test would pass for the wrong reason"
        );

        assert!(
            env_with(Some(dispatched(USER_DEFINED_VARIANT)))
                .resolve_coding_agent(&requested)
                .is_ok()
        );
    }

    /// Absence means "resolve locally". A worker paired with a coordinator that
    /// does not send the field yet, and every single-node caller, must keep
    /// working exactly as before.
    #[test]
    fn nothing_dispatched_falls_back_to_this_node_s_own_profiles() {
        assert!(env_with(None).resolve_coding_agent(&config(None)).is_ok());
    }

    /// The dispatched definition is consulted for the variant that was
    /// requested, not treated as a wholesale replacement for local resolution.
    #[test]
    fn a_dispatched_definition_for_another_variant_does_not_shadow_local_ones() {
        let env = env_with(Some(dispatched(USER_DEFINED_VARIANT)));

        assert!(
            env.resolve_coding_agent(&config(None)).is_ok(),
            "DEFAULT still resolves from this node's profiles"
        );
        assert!(
            env.resolve_coding_agent(&config(Some("NOT_SENT"))).is_err(),
            "a variant neither dispatched nor local is still unknown"
        );
    }

    /// Overrides travel on the action and are applied by whoever resolves it.
    /// The coordinator deliberately sends the *un*-overridden definition, so
    /// this is the only place they get applied.
    #[test]
    fn overrides_on_the_request_survive_a_dispatched_definition() {
        let requested = ExecutorConfig {
            model_id: Some("a-specific-model".into()),
            ..config(Some(USER_DEFINED_VARIANT))
        };
        assert!(requested.has_overrides());

        let agent = env_with(Some(dispatched(USER_DEFINED_VARIANT)))
            .resolve_coding_agent(&requested)
            .unwrap();

        let CodingAgent::ClaudeCode(claude) = agent else {
            panic!("resolved the wrong executor");
        };
        assert_eq!(claude.model.as_deref(), Some("a-specific-model"));
    }
}
