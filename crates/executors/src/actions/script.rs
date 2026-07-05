use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;
use workspace_utils::{command_ext::GroupSpawnNoWindowExt, shell::get_shell_command};

use crate::{
    actions::Executable,
    approvals::ExecutorApprovalService,
    env::ExecutionEnv,
    executors::{ExecutorError, SpawnedChild},
};

/// When set in the execution environment, the script's stdout/stderr are
/// appended to this file instead of piped to the server. This detaches the
/// process from the server's lifetime: it keeps logging even if the server
/// restarts, and the (new) server tails the file. Used for dev servers.
pub const RAW_LOG_PATH_ENV: &str = "VK_RAW_LOG_PATH";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub enum ScriptRequestLanguage {
    Bash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub enum ScriptContext {
    SetupScript,
    CleanupScript,
    ArchiveScript,
    DevServer,
    ToolInstallScript,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
pub struct ScriptRequest {
    pub script: String,
    pub language: ScriptRequestLanguage,
    pub context: ScriptContext,
    /// Optional relative path to execute the script in (relative to container_ref).
    /// If None, uses the container_ref directory directly.
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[async_trait]
impl Executable for ScriptRequest {
    async fn spawn(
        &self,
        current_dir: &Path,
        _approvals: Arc<dyn ExecutorApprovalService>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Use working_dir if specified, otherwise use current_dir
        let effective_dir = match &self.working_dir {
            Some(rel_path) => current_dir.join(rel_path),
            None => current_dir.to_path_buf(),
        };

        let (stdout, stderr) = match env.get(RAW_LOG_PATH_ENV) {
            Some(path) => {
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                let clone = file.try_clone()?;
                (
                    std::process::Stdio::from(clone),
                    std::process::Stdio::from(file),
                )
            }
            None => (std::process::Stdio::piped(), std::process::Stdio::piped()),
        };

        let (shell_cmd, shell_arg) = get_shell_command();
        let mut command = Command::new(shell_cmd);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .arg(shell_arg)
            .arg(&self.script)
            .current_dir(&effective_dir);

        // Apply environment variables
        env.apply_to_command(&mut command);

        let child = command.group_spawn_no_window()?;

        Ok(child.into())
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        approvals::NoopExecutorApprovalService,
        env::{ExecutionEnv, RepoContext},
    };

    #[tokio::test]
    async fn redirects_output_to_raw_log_file_when_env_set() {
        let dir = std::env::temp_dir().join(format!("vk-script-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("out.raw.log");

        let request = ScriptRequest {
            script: "echo hello-raw-log".to_string(),
            language: ScriptRequestLanguage::Bash,
            context: ScriptContext::DevServer,
            working_dir: None,
        };
        let mut env =
            ExecutionEnv::new(RepoContext::new(dir.clone(), vec![]), false, String::new());
        env.insert(RAW_LOG_PATH_ENV, log_path.to_string_lossy().into_owned());

        let mut spawned = request
            .spawn(&dir, Arc::new(NoopExecutorApprovalService), &env)
            .await
            .unwrap();
        // With redirected stdio there are no pipes to consume
        assert!(spawned.child.inner().stdout.is_none());
        spawned.child.wait().await.unwrap();

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("hello-raw-log"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
