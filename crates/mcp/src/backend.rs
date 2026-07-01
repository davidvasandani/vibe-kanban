//! Backend URL resolution for the MCP server.
//!
//! The MCP process translates tool calls into HTTP requests against a locally
//! running Vibe Kanban backend. This module decides which backend to talk to,
//! in priority order:
//!
//! 1. `VIBE_BACKEND_URL` — full URL, used verbatim (preferred, deterministic).
//! 2. `MCP_HOST`/`MCP_PORT` (falling back to `HOST` / `BACKEND_PORT` / `PORT`).
//! 3. The port file written by the running backend.
//!
//! Resolution is intentionally re-runnable: the server re-invokes
//! [`resolve_base_url`] when a request fails with a transient connection error,
//! so a long-lived MCP session self-heals after the backend restarts on a new
//! port instead of staying pinned to a dead one.

use utils::port_file::read_port_file;

const HOST_ENV: &str = "MCP_HOST";
const PORT_ENV: &str = "MCP_PORT";

pub async fn resolve_base_url(log_prefix: &str) -> anyhow::Result<String> {
    if let Ok(url) = std::env::var("VIBE_BACKEND_URL") {
        tracing::info!(
            "[{}] Using backend URL from VIBE_BACKEND_URL: {}",
            log_prefix,
            url
        );
        return Ok(url);
    }

    let host = std::env::var(HOST_ENV)
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "127.0.0.1".to_string());

    let port = match std::env::var(PORT_ENV)
        .or_else(|_| std::env::var("BACKEND_PORT"))
        .or_else(|_| std::env::var("PORT"))
    {
        Ok(port_str) => {
            tracing::info!("[{}] Using port from environment: {}", log_prefix, port_str);
            port_str
                .parse::<u16>()
                .map_err(|error| anyhow::anyhow!("Invalid port value '{}': {}", port_str, error))?
        }
        Err(_) => {
            let port = read_port_file("vibe-kanban").await?;
            tracing::info!("[{}] Using port from port file: {}", log_prefix, port);
            port
        }
    };

    let url = format!("http://{}:{}", host, port);
    tracing::info!("[{}] Using backend URL: {}", log_prefix, url);
    Ok(url)
}
