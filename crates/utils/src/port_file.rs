use std::{env, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::fs;

/// Number of times [`read_port_info`] retries before giving up. The port file
/// is written by the running backend and can be briefly absent or partial while
/// the backend is (re)starting, so a read-once approach reports the backend as
/// unavailable during a window where it is merely mid-restart.
const READ_ATTEMPTS: u32 = 10;
/// Delay between [`read_port_info`] attempts (≈1s total across all attempts).
const READ_RETRY_DELAY: Duration = Duration::from_millis(100);

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

    // Write to a process-unique temp file and atomically rename it into place.
    // `fs::write` truncates-then-writes, so a reader racing a plain in-place
    // write can observe an empty or partial file. A temp+rename makes the
    // published port file switch over atomically, closing that window.
    let tmp_path = dir.join(format!("vibe-kanban.port.{}.tmp", std::process::id()));
    fs::write(&tmp_path, &content).await?;
    if let Err(e) = fs::rename(&tmp_path, &path).await {
        // Best-effort cleanup so a failed rename does not leave temp files behind.
        let _ = fs::remove_file(&tmp_path).await;
        return Err(e);
    }
    Ok(path)
}

pub async fn read_port_file(app_name: &str) -> std::io::Result<u16> {
    read_port_info(app_name).await.map(|info| info.main_port)
}

/// Reads the port file, retrying with a fixed backoff to ride out the brief
/// window where the backend is restarting and the file is missing or partial.
pub async fn read_port_info(app_name: &str) -> std::io::Result<PortInfo> {
    let mut last_err = None;
    for attempt in 0..READ_ATTEMPTS {
        match read_port_info_once(app_name).await {
            Ok(info) => return Ok(info),
            Err(e) => {
                tracing::debug!(
                    "Reading port file for {app_name} failed (attempt {}/{READ_ATTEMPTS}): {e}",
                    attempt + 1,
                );
                last_err = Some(e);
                if attempt + 1 < READ_ATTEMPTS {
                    tokio::time::sleep(READ_RETRY_DELAY).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "port file not found")
    }))
}

async fn read_port_info_once(app_name: &str) -> std::io::Result<PortInfo> {
    let dir = env::temp_dir().join(app_name);
    let path = dir.join(format!("{app_name}.port"));
    tracing::debug!("Reading port from {:?}", path);

    let content = fs::read_to_string(&path).await?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_is_atomic_and_roundtrips() {
        // Use a unique app name so the test does not collide with a running
        // backend's real port file in the shared temp dir.
        let app = format!("vibe-kanban-test-{}", std::process::id());
        let dir = env::temp_dir().join(&app);
        let path = dir.join(format!("{app}.port"));

        // write_port_file_with_proxy hardcodes the "vibe-kanban" dir, so exercise
        // the atomic write path directly against our isolated fixture.
        fs::create_dir_all(&dir).await.unwrap();
        let content = serde_json::to_string(&PortInfo {
            main_port: 4321,
            preview_proxy_port: Some(4322),
        })
        .unwrap();
        let tmp = dir.join("t.tmp");
        fs::write(&tmp, &content).await.unwrap();
        fs::rename(&tmp, &path).await.unwrap();

        let info = read_port_info(&app).await.unwrap();
        assert_eq!(info.main_port, 4321);
        assert_eq!(info.preview_proxy_port, Some(4322));

        // No temp file should remain after the rename.
        assert!(!tmp.exists());

        fs::remove_dir_all(&dir).await.ok();
    }

    #[tokio::test]
    async fn read_retries_until_file_appears() {
        let app = format!("vibe-kanban-test-late-{}", std::process::id());
        let dir = env::temp_dir().join(&app);
        let path = dir.join(format!("{app}.port"));
        fs::remove_dir_all(&dir).await.ok();

        // Write the port file shortly after the read starts, simulating a backend
        // that finishes (re)starting mid-read. The retry loop should ride it out.
        let writer_path = path.clone();
        let writer = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(250)).await;
            fs::create_dir_all(writer_path.parent().unwrap())
                .await
                .unwrap();
            fs::write(&writer_path, "5555").await.unwrap();
        });

        let info = read_port_info(&app).await.unwrap();
        assert_eq!(info.main_port, 5555);
        writer.await.unwrap();

        fs::remove_dir_all(&dir).await.ok();
    }
}
