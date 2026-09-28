use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use worker::{WorkerConfig, run_with_drain};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let config = WorkerConfig::from_env()?;
    let drain_marker = config.state_dir.join("release-admission-drain");
    let shutdown = CancellationToken::new();
    let admission_draining = Arc::new(AtomicBool::new(tokio::fs::try_exists(&drain_marker).await?));
    let signal = shutdown.clone();
    let signal_draining = admission_draining.clone();
    let signal_drain_marker = drain_marker.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal as unix_signal};

            let mut terminate = unix_signal(SignalKind::terminate()).ok();
            let mut drain = unix_signal(SignalKind::user_defined1()).ok();
            let mut resume = unix_signal(SignalKind::user_defined2()).ok();
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        signal.cancel();
                        return;
                    }
                    _ = async { terminate.as_mut().unwrap().recv().await }, if terminate.is_some() => {
                        signal.cancel();
                        return;
                    }
                    _ = async { drain.as_mut().unwrap().recv().await }, if drain.is_some() => {
                        if let Err(error) = tokio::fs::write(&signal_drain_marker, b"draining\n").await {
                            tracing::error!(?error, "failed to persist worker admission drain marker");
                            continue;
                        }
                        signal_draining.store(true, Ordering::Release);
                        tracing::info!("worker admission paused for release handoff");
                    }
                    _ = async { resume.as_mut().unwrap().recv().await }, if resume.is_some() => {
                        match tokio::fs::remove_file(&signal_drain_marker).await {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => {
                                tracing::error!(?error, "failed to clear worker admission drain marker");
                                continue;
                            }
                        }
                        signal_draining.store(false, Ordering::Release);
                        tracing::info!("worker admission resumed after deferred release handoff");
                    }
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            signal.cancel();
        }
    });
    // The coordinator's CLI Tools page checks the coordinator; only this host
    // can say whether its own agents can run the shared managed tools. A
    // diagnostic only: an unusable optional CLI must never keep the worker out
    // of scheduling.
    tokio::spawn(log_agent_tool_readiness());
    run_with_drain(config, shutdown, admission_draining).await
}

async fn log_agent_tool_readiness() {
    use utils::agent_tools::{AgentToolState, check_managed_bin};

    let results = check_managed_bin().await;
    let mut usable = 0;
    for (tool, check) in &results {
        if check.state == AgentToolState::Available {
            usable += 1;
            continue;
        }
        tracing::warn!(
            tool = %tool,
            state = ?check.state,
            path = ?check.path,
            login_path = ?check.login_path,
            reason = check.message.as_deref().unwrap_or(""),
            "managed CLI tool is not usable by agents on this worker"
        );
    }
    tracing::info!(
        usable,
        total = results.len(),
        "checked managed CLI tools in the agent environment"
    );
}
