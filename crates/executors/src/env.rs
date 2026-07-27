use std::{collections::HashMap, path::PathBuf};

use git::GitService;
use tokio::process::Command;

use crate::command::CmdOverrides;

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
        }
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
    ///
    /// Profile values override runtime values except for `PATH`, where profile
    /// entries take precedence but runtime entries remain available. Runtime
    /// `PATH` can contain execution-owned directories such as the app-managed
    /// CLI tools bin directory, which a profile must not erase.
    pub fn with_profile(mut self, cmd: &CmdOverrides) -> Self {
        if let Some(ref profile_env) = cmd.env {
            let runtime_path = env_path(&self.vars).cloned();
            let profile_path = env_path(profile_env).cloned();
            self.merge(profile_env);

            if let Some(profile_path) = profile_path {
                #[cfg(windows)]
                self.vars.retain(|key, _| !is_path_key(key));

                let effective_path = if let Some(runtime_path) = runtime_path {
                    workspace_utils::shell::merge_paths(profile_path, runtime_path)
                } else {
                    profile_path.into()
                };
                self.insert("PATH", effective_path.to_string_lossy().into_owned());
            }
        }

        self
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

fn env_path(env: &HashMap<String, String>) -> Option<&String> {
    env.iter()
        .find_map(|(key, value)| is_path_key(key).then_some(value))
}

#[cfg(not(windows))]
fn is_path_key(key: &str) -> bool {
    key == "PATH"
}

#[cfg(windows)]
fn is_path_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("PATH")
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

    #[test]
    fn profile_path_keeps_runtime_owned_entries() {
        let mut base = ExecutionEnv::new(RepoContext::default(), false, String::new());
        let runtime_path =
            std::env::join_paths(["/runtime/bin", "/managed/cli-tools/bin"]).unwrap();
        base.insert("PATH", runtime_path.to_string_lossy());

        let mut profile = HashMap::new();
        let profile_path = std::env::join_paths(["/profile/bin", "/runtime/bin"]).unwrap();
        profile.insert(
            "PATH".to_string(),
            profile_path.to_string_lossy().into_owned(),
        );

        let merged = base.with_profile(&CmdOverrides {
            env: Some(profile),
            ..Default::default()
        });
        let paths: Vec<_> = std::env::split_paths(merged.vars.get("PATH").unwrap()).collect();

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/profile/bin"),
                PathBuf::from("/runtime/bin"),
                PathBuf::from("/managed/cli-tools/bin"),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn profile_path_casing_is_normalized_on_windows() {
        let mut base = ExecutionEnv::new(RepoContext::default(), false, String::new());
        base.insert("PATH", r"C:\runtime");

        let mut profile = HashMap::new();
        profile.insert("Path".to_string(), r"C:\profile".to_string());

        let merged = base.with_profile(&CmdOverrides {
            env: Some(profile),
            ..Default::default()
        });

        assert_eq!(
            merged
                .vars
                .keys()
                .filter(|key| key.eq_ignore_ascii_case("PATH"))
                .count(),
            1
        );
        assert_eq!(
            std::env::split_paths(merged.vars.get("PATH").unwrap()).collect::<Vec<_>>(),
            vec![PathBuf::from(r"C:\profile"), PathBuf::from(r"C:\runtime")]
        );
    }
}
