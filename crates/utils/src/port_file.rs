use std::{env, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::fs;

/// Number of attempts when reading the port file. The backend rewrites the file
/// on every (re)start, and a reader launched during that window can observe a
/// missing, empty, or partially-written file. Retrying covers that race.
const READ_ATTEMPTS: u32 = 10;
const READ_RETRY_DELAY: Duration = Duration::from_millis(150);

#[derive(Debug, Serialize, Deserialize)]
pub struct PortInfo {
    pub main_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_proxy_port: Option<u16>,
}

pub async fn write_port_file_with_proxy(
    main_port: u16,
    preview_proxy_port: Option<u16>,
) -> std::io::Result<PathBuf> {
    let dir = env::temp_dir().join("vibe-kanban");
    let path = dir.join("vibe-kanban.port");
    let port_info = PortInfo {
        main_port,
        preview_proxy_port,
    };
    let content = serde_json::to_string(&port_info)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    tracing::debug!("Writing ports {:?} to {:?}", port_info, path);
    fs::create_dir_all(&dir).await?;
    // Write atomically: a plain `fs::write` truncates then writes, leaving a
    // sub-second window where a concurrent reader sees an empty/partial file
    // (and the MCP server then exits). Write to a unique temp file and rename
    // into place — rename is atomic on the same filesystem.
    let tmp_path = dir.join(format!("vibe-kanban.port.{}.tmp", std::process::id()));
    fs::write(&tmp_path, content).await?;
    fs::rename(&tmp_path, &path).await?;
    Ok(path)
}

pub async fn read_port_file(app_name: &str) -> std::io::Result<u16> {
    read_port_info(app_name).await.map(|info| info.main_port)
}

/// Read the port file, retrying briefly to ride out the window where the backend
/// is (re)writing it. A single read can land on a missing, empty, or partially
/// written file; without retry the caller (e.g. the stdio MCP server) exits and
/// the client reports "failed to connect" / intermittent availability.
pub async fn read_port_info(app_name: &str) -> std::io::Result<PortInfo> {
    let mut last_err = None;
    for attempt in 0..READ_ATTEMPTS {
        match read_port_info_once(app_name).await {
            Ok(info) => return Ok(info),
            Err(e) => {
                tracing::debug!(
                    "Port file read attempt {}/{} failed: {}",
                    attempt + 1,
                    READ_ATTEMPTS,
                    e
                );
                last_err = Some(e);
                if attempt + 1 < READ_ATTEMPTS {
                    tokio::time::sleep(READ_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "port file not available")
    }))
}

async fn read_port_info_once(app_name: &str) -> std::io::Result<PortInfo> {
    let dir = env::temp_dir().join(app_name);
    let path = dir.join(format!("{app_name}.port"));
    tracing::debug!("Reading port from {:?}", path);

    let content = fs::read_to_string(&path).await?;

    // Treat an empty file (mid-write window) as a retryable miss rather than a
    // hard parse error.
    if content.trim().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "port file is empty",
        ));
    }

    if let Ok(port_info) = serde_json::from_str::<PortInfo>(&content) {
        return Ok(port_info);
    }

    let port: u16 = content
        .trim()
        .parse()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(PortInfo {
        main_port: port,
        preview_proxy_port: None,
    })
}
