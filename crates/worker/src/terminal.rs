use std::{
    collections::HashMap,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use cluster_protocol::{TerminalCreateRequest, TerminalOutputBatch};
use portable_pty::{ChildKiller, CommandBuilder, NativePtySystem, PtySize, PtySystem};
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::path_authority::{PathAuthority, PathAuthorityError};

const OUTPUT_QUEUE_CAPACITY: usize = 128;
const MAX_TERMINAL_DIMENSION: u16 = 1_000;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error(transparent)]
    Path(#[from] PathAuthorityError),
    #[error("terminal {0} was not found")]
    NotFound(Uuid),
    #[error("terminal operation failed: {0}")]
    Operation(String),
}

struct Session {
    writer: Box<dyn Write + Send>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    output: mpsc::Receiver<Vec<u8>>,
}

#[derive(Clone)]
pub struct TerminalService {
    paths: PathAuthority,
    sessions: Arc<Mutex<HashMap<Uuid, Session>>>,
}

impl TerminalService {
    pub fn new(paths: PathAuthority) -> Self {
        Self {
            paths,
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create(&self, request: TerminalCreateRequest) -> Result<Uuid, TerminalError> {
        validate_size(request.cols, request.rows)?;
        let workspace = self
            .paths
            .authorize_workspace_path(&request.workspace_path)?;
        let working_directory = PathBuf::from(&request.working_directory);
        if !working_directory.starts_with(&workspace) {
            return Err(TerminalError::Operation(
                "working directory is outside workspace".into(),
            ));
        }
        let terminal_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(OUTPUT_QUEUE_CAPACITY);
        let session = tokio::task::spawn_blocking(move || {
            let pair = NativePtySystem::default()
                .openpty(PtySize {
                    rows: request.rows,
                    cols: request.cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(operation_error)?;
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
            let mut command = CommandBuilder::new(shell);
            command.cwd(working_directory);
            for (key, value) in request.environment {
                command.env(key, value);
            }
            command.env("TERM", "xterm-256color");
            command.env("VIBE_KANBAN_TERMINAL", "1");
            let mut child = pair.slave.spawn_command(command).map_err(operation_error)?;
            let killer = child.clone_killer();
            let writer = pair.master.take_writer().map_err(operation_error)?;
            let mut reader = pair.master.try_clone_reader().map_err(operation_error)?;
            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                while let Ok(size) = reader.read(&mut buffer) {
                    if size == 0 || tx.blocking_send(buffer[..size].to_vec()).is_err() {
                        break;
                    }
                }
                let _ = child.wait();
            });
            Ok::<_, TerminalError>(Session {
                writer,
                master: pair.master,
                killer,
                output: rx,
            })
        })
        .await
        .map_err(operation_error)??;
        self.sessions
            .lock()
            .map_err(lock_error)?
            .insert(terminal_id, session);
        Ok(terminal_id)
    }

    pub fn input(&self, terminal_id: Uuid, bytes: &[u8]) -> Result<(), TerminalError> {
        let mut sessions = self.sessions.lock().map_err(lock_error)?;
        let session = sessions
            .get_mut(&terminal_id)
            .ok_or(TerminalError::NotFound(terminal_id))?;
        session.writer.write_all(bytes).map_err(operation_error)?;
        session.writer.flush().map_err(operation_error)
    }

    pub fn resize(&self, terminal_id: Uuid, cols: u16, rows: u16) -> Result<(), TerminalError> {
        validate_size(cols, rows)?;
        let sessions = self.sessions.lock().map_err(lock_error)?;
        let session = sessions
            .get(&terminal_id)
            .ok_or(TerminalError::NotFound(terminal_id))?;
        session
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(operation_error)
    }

    pub fn output(&self, terminal_id: Uuid) -> Result<TerminalOutputBatch, TerminalError> {
        let mut sessions = self.sessions.lock().map_err(lock_error)?;
        let session = sessions
            .get_mut(&terminal_id)
            .ok_or(TerminalError::NotFound(terminal_id))?;
        let mut chunks_base64 = Vec::new();
        while let Ok(chunk) = session.output.try_recv() {
            chunks_base64.push(BASE64_STANDARD.encode(chunk));
        }
        let closed = session.output.is_closed() && session.output.is_empty();
        Ok(TerminalOutputBatch {
            terminal_id,
            chunks_base64,
            closed,
        })
    }

    pub fn close(&self, terminal_id: Uuid) -> Result<(), TerminalError> {
        if let Some(mut session) = self
            .sessions
            .lock()
            .map_err(lock_error)?
            .remove(&terminal_id)
        {
            let _ = session.killer.kill();
        }
        Ok(())
    }
}

fn lock_error(error: impl ToString) -> TerminalError {
    TerminalError::Operation(error.to_string())
}
fn operation_error(error: impl ToString) -> TerminalError {
    TerminalError::Operation(error.to_string())
}

fn validate_size(cols: u16, rows: u16) -> Result<(), TerminalError> {
    if cols == 0 || rows == 0 || cols > MAX_TERMINAL_DIMENSION || rows > MAX_TERMINAL_DIMENSION {
        return Err(TerminalError::Operation(
            "terminal dimensions are out of range".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_dimensions_are_bounded() {
        assert!(validate_size(80, 24).is_ok());
        assert!(validate_size(0, 24).is_err());
        assert!(validate_size(80, MAX_TERMINAL_DIMENSION + 1).is_err());
    }
}
