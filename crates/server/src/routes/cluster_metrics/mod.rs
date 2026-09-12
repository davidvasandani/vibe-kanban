//! The browser-facing surface for cluster metrics.
//!
//! Two routes, both read-only. Neither has a mutating verb and neither carries
//! a field the client can send that influences what a host reads, which is what
//! keeps "observability cannot influence lifecycle state" (constitution XIX) a
//! property of the API rather than a promise about the handlers.
//!
//! The interesting requirements are both about *not* leaking work:
//!
//! - The REST route runs **one bounded collection round if the collector is
//!   idle**, then serves. Without it, a cluster whose drawer has been shut for
//!   longer than the retention window answers `latest: null` for every worker —
//!   a fallback that falls back to nothing (analysis W2). It does not start the
//!   continuous collector.
//! - The WS route holds a [`MetricsSubscription`] for the socket's lifetime, so
//!   the slot is released on a clean close, an abnormal close, and a panic
//!   alike, and it enforces a liveness deadline: a half-open TCP connection
//!   never delivers a close, so close detection alone would pin the collector
//!   on forever while the reconnecting client increments a *new* subscriber
//!   (analysis W5).

mod patch;

use std::time::Duration;

use axum::{
    Router,
    extract::{State, ws::Message},
    response::{IntoResponse, Json as ResponseJson},
    routing::get,
};
use deployment::Deployment;
use services::services::cluster::{ClusterMetricsError, ClusterMetricsSnapshot};
use utils::{log_msg::LogMsg, response::ApiResponse};

use self::patch::MetricsPatchBuilder;
use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
};

/// How often the server asks the peer to prove it is still there.
const PING_INTERVAL: Duration = Duration::from_secs(15);
/// How long a ping may go unanswered before the connection — and its
/// subscriber slot — is dropped.
const PONG_DEADLINE: Duration = Duration::from_secs(10);
/// A send that cannot complete in this long is a stalled peer, not a slow one.
/// This is the liveness bound on the relayed path, where WebSocket control
/// frames are not observable; see [`handle_cluster_metrics_ws`].
const SEND_TIMEOUT: Duration = Duration::from_secs(20);
/// Floor on the streaming cadence, so a misconfigured interval cannot turn the
/// socket into a busy loop.
const MIN_TICK: Duration = Duration::from_millis(250);

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/cluster/metrics", get(get_cluster_metrics))
        .route("/cluster/metrics/ws", get(stream_cluster_metrics_ws))
}

fn map_error(error: ClusterMetricsError) -> ApiError {
    match error {
        ClusterMetricsError::Database(error) => ApiError::Database(error),
    }
}

/// The snapshot / fallback path.
async fn get_cluster_metrics(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ClusterMetricsSnapshot>>, ApiError> {
    let metrics = deployment.cluster_metrics();
    // One bounded round, and only if nobody is already collecting. This is what
    // makes the route a real fallback rather than a reader of an empty cache.
    metrics.collect_once_if_idle().await;
    let snapshot = metrics.snapshot().await.map_err(map_error)?;
    Ok(ResponseJson(ApiResponse::success(snapshot)))
}

/// The live path: snapshot, then JSON-Patch, over the signed socket.
async fn stream_cluster_metrics_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(error) = handle_cluster_metrics_ws(socket, deployment).await {
            tracing::warn!("cluster metrics WS closed: {}", error);
        }
    })
}

async fn handle_cluster_metrics_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
) -> anyhow::Result<()> {
    let service = deployment.cluster_metrics().clone();
    // Held for the whole handler. Dropping it is what releases the slot, so a
    // clean close, an abnormal close, a panic and a cancelled task all release
    // it identically — there is no branch anyone has to remember to write.
    let _subscription = service.subscribe();

    let mut builder = MetricsPatchBuilder::new();

    // Snapshot first, then patches, mirroring `stream_approvals_ws`.
    let snapshot = service.snapshot().await?;
    send(&mut socket, LogMsg::JsonPatch(builder.next(&snapshot))).await?;
    send(&mut socket, LogMsg::Ready).await?;

    let mut ticker =
        tokio::time::interval(Duration::from_millis(service.sample_interval_ms()).max(MIN_TICK));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // The first tick of a `tokio` interval resolves immediately, and the
    // snapshot above already covered it.
    ticker.tick().await;

    let mut ping_ticker = tokio::time::interval(PING_INTERVAL);
    ping_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ping_ticker.tick().await;

    // On a relayed connection every frame is wrapped in a signed envelope and
    // the wrapper drops control frames on receipt, so a pong can never reach
    // this loop; sending a ping there would guarantee the deadline below fires
    // on a perfectly healthy socket. That path is bounded by [`SEND_TIMEOUT`]
    // instead, which a stalled peer trips once its receive window fills.
    let control_frames_observable = !socket.is_signed();
    let mut pong_deadline: Option<tokio::time::Instant> = None;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let snapshot = match service.snapshot().await {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        // A transient read failure is not a stream error; the
                        // next tick resnapshots.
                        tracing::debug!(%error, "cluster metrics: snapshot unavailable this tick");
                        continue;
                    }
                };
                let patch = builder.next(&snapshot);
                // A tick in which no node produced a sample costs nothing.
                if patch.0.is_empty() {
                    continue;
                }
                if send(&mut socket, LogMsg::JsonPatch(patch)).await.is_err() {
                    break;
                }
            }
            _ = ping_ticker.tick(), if control_frames_observable => {
                if pong_deadline.is_none() {
                    if send_frame(&mut socket, Message::Ping(Default::default())).await.is_err() {
                        break;
                    }
                    pong_deadline = Some(tokio::time::Instant::now() + PONG_DEADLINE);
                }
            }
            _ = expire(pong_deadline) => {
                tracing::warn!(
                    "cluster metrics WS: no pong within {:?}, dropping half-open connection",
                    PONG_DEADLINE
                );
                break;
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) | Ok(None) => break,
                    // Any frame at all is evidence the peer is still there.
                    Ok(Some(_)) => pong_deadline = None,
                    Err(error) => {
                        tracing::warn!("cluster metrics WS receive error: {}", error);
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Resolves at `deadline`, or never when there is none.
async fn expire(deadline: Option<tokio::time::Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

async fn send(socket: &mut MaybeSignedWebSocket, message: LogMsg) -> anyhow::Result<()> {
    send_frame(socket, message.to_ws_message_unchecked()).await
}

/// Every send is bounded. An unbounded one on a half-open connection parks this
/// task forever, and with it the subscriber slot the collector is gated on.
async fn send_frame(socket: &mut MaybeSignedWebSocket, message: Message) -> anyhow::Result<()> {
    match tokio::time::timeout(SEND_TIMEOUT, socket.send(message)).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "cluster metrics WS send stalled for {:?}",
            SEND_TIMEOUT
        )),
    }
}
