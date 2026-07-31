use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;
use worker::{WorkerConfig, run};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let shutdown = CancellationToken::new();
    let signal = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        signal.cancel();
    });
    run(WorkerConfig::from_env()?, shutdown).await
}
