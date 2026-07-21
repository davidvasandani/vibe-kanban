use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};

use portable_pty::{ChildKiller, CommandBuilder, NativePtySystem, PtySize, PtySystem};
use thiserror::Error;
use tokio::sync::mpsc;
use utils::shell::get_interactive_shell;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PtyError {
    #[error("Failed to create PTY: {0}")]
    CreateFailed(String),
    #[error("Session not found: {0}")]
    SessionNotFound(Uuid),
    #[error("Failed to write to PTY: {0}")]
    WriteFailed(String),
    #[error("Failed to resize PTY: {0}")]
    ResizeFailed(String),
    #[error("Session already closed")]
    SessionClosed,
}

struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    _output_handle: thread::JoinHandle<()>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyExit {
    pub code: u32,
}

#[derive(Clone)]
pub struct PtyService {
    sessions: Arc<Mutex<HashMap<Uuid, PtySession>>>,
}

impl PtyService {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_session(
        &self,
        working_dir: PathBuf,
        environment: HashMap<String, String>,
        cols: u16,
        rows: u16,
    ) -> Result<(Uuid, mpsc::UnboundedReceiver<Vec<u8>>), PtyError> {
        let shell = get_interactive_shell().await;

        let (session_id, output_rx, _exit_rx) = self
            .create_command_session(
                shell,
                Vec::new(),
                working_dir,
                environment,
                cols,
                rows,
                true,
            )
            .await?;
        Ok((session_id, output_rx))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_command_session(
        &self,
        executable: PathBuf,
        args: Vec<String>,
        working_dir: PathBuf,
        environment: HashMap<String, String>,
        cols: u16,
        rows: u16,
        interactive_shell: bool,
    ) -> Result<
        (
            Uuid,
            mpsc::UnboundedReceiver<Vec<u8>>,
            mpsc::UnboundedReceiver<PtyExit>,
        ),
        PtyError,
    > {
        let session_id = Uuid::new_v4();
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        let (exit_tx, exit_rx) = mpsc::unbounded_channel();

        let result = tokio::task::spawn_blocking(move || {
            let pty_system = NativePtySystem::default();

            let pty_pair = pty_system
                .openpty(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            let mut cmd = CommandBuilder::new(&executable);
            cmd.cwd(&working_dir);
            for arg in args {
                cmd.arg(arg);
            }

            if !interactive_shell {
                cmd.env_clear();
                for key in [
                    "HOME",
                    "USER",
                    "PATH",
                    "TMPDIR",
                    "TEMP",
                    "LANG",
                    "LC_ALL",
                    "SSL_CERT_FILE",
                    "SSL_CERT_DIR",
                    "HTTP_PROXY",
                    "HTTPS_PROXY",
                    "ALL_PROXY",
                    "NO_PROXY",
                    "http_proxy",
                    "https_proxy",
                    "all_proxy",
                    "no_proxy",
                    "REQUESTS_CA_BUNDLE",
                    "CURL_CA_BUNDLE",
                    "XDG_CONFIG_HOME",
                    "AZURE_CONFIG_DIR",
                    "GAMCFGDIR",
                ] {
                    if let Some(value) = std::env::var_os(key) {
                        cmd.env(key, value);
                    }
                }
            }

            // Apply workspace-scoped values before the PTY's own contract so
            // terminal/runtime-owned values below always take precedence.
            for (key, value) in environment {
                cmd.env(key, value);
            }

            // Configure shell-specific options
            let shell_name = executable
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if interactive_shell && (shell_name == "powershell.exe" || shell_name == "pwsh.exe") {
                // PowerShell: use -NoLogo for cleaner startup
                cmd.arg("-NoLogo");
            } else if interactive_shell && shell_name == "cmd.exe" {
                // cmd.exe: no special args needed
            } else if interactive_shell {
                // Unix shells
                cmd.env("VIBE_KANBAN_TERMINAL", "1");

                if shell_name == "bash" {
                    cmd.env("PROMPT_COMMAND", r#"PS1='$ '; unset PROMPT_COMMAND"#);
                } else if shell_name == "zsh" {
                    // PROMPT is set after spawning
                } else {
                    cmd.env("PS1", "$ ");
                }
            }

            cmd.env("TERM", "xterm-256color");
            cmd.env("COLORTERM", "truecolor");

            let mut child = pty_pair
                .slave
                .spawn_command(cmd)
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;
            let killer = child.clone_killer();

            let mut writer = pty_pair
                .master
                .take_writer()
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            if interactive_shell && shell_name == "zsh" {
                let _ = writer.write_all(b" PROMPT='$ '; RPROMPT=''\n");
                let _ = writer.flush();
                let _ = writer.write_all(b"\x0c");
                let _ = writer.flush();
            }

            let mut reader = pty_pair
                .master
                .try_clone_reader()
                .map_err(|e| PtyError::CreateFailed(e.to_string()))?;

            let output_handle = thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if output_tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let code = child.wait().map(|status| status.exit_code()).unwrap_or(1);
                let _ = exit_tx.send(PtyExit { code });
            });

            Ok::<_, PtyError>((pty_pair.master, writer, output_handle, killer))
        })
        .await
        .map_err(|e| PtyError::CreateFailed(e.to_string()))??;

        let (master, writer, output_handle, killer) = result;

        let session = PtySession {
            writer,
            master,
            _output_handle: output_handle,
            killer,
            closed: false,
        };

        self.sessions
            .lock()
            .map_err(|e| PtyError::CreateFailed(e.to_string()))?
            .insert(session_id, session);

        Ok((session_id, output_rx, exit_rx))
    }

    pub async fn write(&self, session_id: Uuid, data: &[u8]) -> Result<(), PtyError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;
        let session = sessions
            .get_mut(&session_id)
            .ok_or(PtyError::SessionNotFound(session_id))?;

        if session.closed {
            return Err(PtyError::SessionClosed);
        }

        session
            .writer
            .write_all(data)
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;

        session
            .writer
            .flush()
            .map_err(|e| PtyError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn resize(&self, session_id: Uuid, cols: u16, rows: u16) -> Result<(), PtyError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|e| PtyError::ResizeFailed(e.to_string()))?;
        let session = sessions
            .get(&session_id)
            .ok_or(PtyError::SessionNotFound(session_id))?;

        if session.closed {
            return Err(PtyError::SessionClosed);
        }

        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::ResizeFailed(e.to_string()))?;

        Ok(())
    }

    pub async fn close_session(&self, session_id: Uuid) -> Result<(), PtyError> {
        if let Some(mut session) = self
            .sessions
            .lock()
            .map_err(|_| PtyError::SessionClosed)?
            .remove(&session_id)
        {
            session.closed = true;
            let _ = session.killer.kill();
        }
        Ok(())
    }

    /// Remove a session whose child has already been reaped by the output
    /// thread. Unlike `close_session`, this must not signal the cloned PID.
    pub async fn finish_session(&self, session_id: Uuid) -> Result<(), PtyError> {
        self.sessions
            .lock()
            .map_err(|_| PtyError::SessionClosed)?
            .remove(&session_id);
        Ok(())
    }
}

impl Default for PtyService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_session_reports_exit_and_output() {
        let service = PtyService::new();
        let (id, mut output, mut exit) = service
            .create_command_session(
                PathBuf::from("/bin/sh"),
                vec!["-c".into(), "printf login-ok".into()],
                std::env::temp_dir(),
                HashMap::new(),
                80,
                24,
                false,
            )
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Ok(Some(chunk)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), output.recv()).await
        {
            bytes.extend(chunk);
        }
        let status = tokio::time::timeout(std::time::Duration::from_secs(2), exit.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.code, 0);
        assert!(String::from_utf8_lossy(&bytes).contains("login-ok"));
        service.close_session(id).await.unwrap();
    }

    #[tokio::test]
    async fn command_session_receives_environment_without_overriding_terminal_contract() {
        let service = PtyService::new();
        let environment = HashMap::from([
            ("ORG_TEST_VALUE".to_string(), "available".to_string()),
            ("TERM".to_string(), "org-value".to_string()),
        ]);
        let (id, mut output, mut exit) = service
            .create_command_session(
                PathBuf::from("/bin/sh"),
                vec![
                    "-c".into(),
                    "printf '%s|%s' \"$ORG_TEST_VALUE\" \"$TERM\"".into(),
                ],
                std::env::temp_dir(),
                environment,
                80,
                24,
                false,
            )
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Ok(Some(chunk)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), output.recv()).await
        {
            bytes.extend(chunk);
        }
        let status = tokio::time::timeout(std::time::Duration::from_secs(2), exit.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status.code, 0);
        assert!(String::from_utf8_lossy(&bytes).contains("available|xterm-256color"));
        service.close_session(id).await.unwrap();
    }
}
