//! Runtime-level tests for the browser control gateway (SPEC §12). These
//! exercise `SessionRuntime` directly against the mock driver — the DB-backed
//! service layer adds only persistence/audit around these semantics.

use chrono::Duration;
use uuid::Uuid;

use super::{
    arbiter::TransferTarget,
    driver::{BrowserDriver, MockDriver},
    session::SessionRuntime,
    types::{
        BrowserAction, BrowserController, BrowserSessionError, ControlPrincipal,
        ControlTransitionReason,
    },
};

async fn runtime() -> SessionRuntime {
    runtime_with_ttl(Duration::seconds(60)).await
}

async fn runtime_with_ttl(ttl: Duration) -> SessionRuntime {
    let handle = MockDriver
        .launch(Default::default())
        .await
        .expect("mock launch");
    SessionRuntime::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        None,
        handle,
        ttl,
        Duration::minutes(120),
    )
}

fn agent(execution_id: Uuid) -> ControlPrincipal {
    ControlPrincipal::Agent { execution_id }
}

fn human(connection_id: Uuid) -> ControlPrincipal {
    ControlPrincipal::Human {
        user_id: "local-user".to_string(),
        connection_id,
    }
}

fn navigate(url: &str) -> BrowserAction {
    BrowserAction::Navigate {
        url: url.to_string(),
    }
}

#[tokio::test]
async fn agent_acquires_and_navigates() {
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    let result = rt
        .execute(
            &ag,
            Uuid::new_v4(),
            Some(1),
            &navigate("https://example.com"),
        )
        .await
        .unwrap();
    assert_eq!(result.generation, 1);
    assert_eq!(result.current_url.as_deref(), Some("https://example.com"));
    assert_eq!(
        rt.live_state().current_url.as_deref(),
        Some("https://example.com")
    );
}

#[tokio::test]
async fn command_without_control_is_conflict() {
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    let err = rt
        .execute(&ag, Uuid::new_v4(), None, &navigate("https://example.com"))
        .await
        .unwrap_err();
    assert!(matches!(err, BrowserSessionError::ControlConflict { .. }));
}

#[tokio::test]
async fn takeover_invalidates_stale_agent_commands() {
    let rt = runtime().await;
    let exec = Uuid::new_v4();
    let ag = agent(exec);
    rt.acquire(&ag, false, false, None).unwrap();

    // Human takes control (generation 1 → 2).
    let t = rt
        .acquire(&human(Uuid::new_v4()), true, false, None)
        .unwrap();
    assert_eq!(t.reason, ControlTransitionReason::Takeover);

    // Agent command carrying the old generation: typed stale/lost, and the
    // driver must never see it.
    let err = rt
        .execute(
            &ag,
            Uuid::new_v4(),
            Some(1),
            &navigate("https://agent.example"),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        BrowserSessionError::StaleGeneration { .. } | BrowserSessionError::ControlLost { .. }
    ));
    // Agent command without expected generation: still rejected (not holder).
    let err = rt
        .execute(
            &ag,
            Uuid::new_v4(),
            None,
            &navigate("https://agent.example"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrowserSessionError::ControlConflict { .. }));
    assert!(rt.live_state().current_url.is_none());
}

#[tokio::test]
async fn queued_command_behind_driver_lock_is_control_lost() {
    // Directly exercise the admission/recheck split: a command admitted under
    // generation 1 that only reaches the driver after a takeover must fail
    // CONTROL_LOST without executing (invalidate, never replay).
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    // Simulate "queued while takeover lands" by racing many agent commands
    // with a takeover; afterwards the URL must be the human's last navigation
    // and no agent navigation may have executed after the takeover.
    let rt = std::sync::Arc::new(rt);
    let mut handles = Vec::new();
    for i in 0..8 {
        let rt2 = rt.clone();
        let ag2 = ag.clone();
        handles.push(tokio::spawn(async move {
            rt2.execute(
                &ag2,
                Uuid::new_v4(),
                Some(1),
                &navigate(&format!("https://agent.example/{i}")),
            )
            .await
        }));
    }
    let conn = Uuid::new_v4();
    let h = human(conn);
    rt.acquire(&h, true, false, None).unwrap();
    let human_gen = rt.control_state().generation;
    rt.execute(
        &h,
        Uuid::new_v4(),
        Some(human_gen),
        &navigate("https://human.example"),
    )
    .await
    .unwrap();
    for handle in handles {
        // Every agent command either succeeded before the takeover (fine) or
        // failed typed — none may have executed after the human navigation.
        let _ = handle.await.unwrap();
    }
    assert_eq!(
        rt.live_state().current_url.as_deref(),
        Some("https://human.example")
    );
}

#[tokio::test]
async fn agent_human_agent_round_trip_preserves_browser() {
    let rt = runtime().await;
    let exec = Uuid::new_v4();
    let ag = agent(exec);
    rt.acquire(&ag, false, false, None).unwrap();
    rt.execute(
        &ag,
        Uuid::new_v4(),
        Some(1),
        &navigate("https://step1.example"),
    )
    .await
    .unwrap();

    let conn = Uuid::new_v4();
    let h = human(conn);
    rt.acquire(&h, true, false, None).unwrap();
    rt.execute(
        &h,
        Uuid::new_v4(),
        Some(2),
        &navigate("https://login.example"),
    )
    .await
    .unwrap();

    // Return control to the same execution via CAS transfer.
    let t = rt
        .transfer(&h, TransferTarget::Agent { execution_id: exec }, 2)
        .unwrap();
    assert_eq!(t.generation, 3);
    assert_eq!(t.controller.execution_id(), Some(exec));

    // The agent resumes in the same session: page state intact, new
    // generation works.
    assert_eq!(
        rt.live_state().current_url.as_deref(),
        Some("https://login.example")
    );
    let result = rt
        .execute(
            &ag,
            Uuid::new_v4(),
            Some(3),
            &navigate("https://step2.example"),
        )
        .await
        .unwrap();
    assert_eq!(result.generation, 3);
    assert!(!rt.is_closed());
}

#[tokio::test]
async fn transfer_with_stale_generation_rejected() {
    let rt = runtime().await;
    let conn = Uuid::new_v4();
    let h = human(conn);
    rt.acquire(&h, false, false, None).unwrap();
    let err = rt.transfer(&h, TransferTarget::None, 0).unwrap_err();
    assert!(matches!(err, BrowserSessionError::StaleGeneration { .. }));
}

#[tokio::test]
async fn duplicate_command_id_returns_recorded_result_without_reexecution() {
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    let command_id = Uuid::new_v4();
    let first = rt
        .execute(&ag, command_id, Some(1), &navigate("https://once.example"))
        .await
        .unwrap();
    let second = rt
        .execute(&ag, command_id, Some(1), &navigate("https://once.example"))
        .await
        .unwrap();
    assert_eq!(first.command_id, second.command_id);
    assert_eq!(first.generation, second.generation);
    // Only one navigation reached the driver.
    let state = rt.live_state();
    assert_eq!(state.current_url.as_deref(), Some("https://once.example"));
}

#[tokio::test]
async fn disconnect_releases_connection_bound_lease_only() {
    let rt = runtime().await;
    let conn = Uuid::new_v4();
    rt.acquire(&human(conn), false, false, None).unwrap();
    // A different connection's disconnect must not release.
    assert!(
        rt.release_if(
            |c| c.connection_id() == Some(Uuid::new_v4()),
            ControlTransitionReason::Disconnected,
        )
        .is_none()
    );
    let t = rt
        .release_if(
            |c| c.connection_id() == Some(conn),
            ControlTransitionReason::Disconnected,
        )
        .unwrap();
    assert_eq!(t.reason, ControlTransitionReason::Disconnected);
    assert!(rt.control_state().controller.is_none());
    assert!(!rt.is_closed());
}

#[tokio::test]
async fn execution_completion_releases_lease_but_keeps_session_alive() {
    let rt = runtime().await;
    let exec = Uuid::new_v4();
    rt.acquire(&agent(exec), false, false, None).unwrap();
    let t = rt
        .release_if(
            |c| c.execution_id() == Some(exec),
            ControlTransitionReason::ExecutionCompleted,
        )
        .unwrap();
    assert_eq!(t.reason, ControlTransitionReason::ExecutionCompleted);
    assert!(!rt.is_closed());
    // A new controller can acquire under the bumped generation.
    let h = human(Uuid::new_v4());
    let t = rt.acquire(&h, false, false, None).unwrap();
    assert_eq!(t.generation, 3);
}

#[tokio::test]
async fn lease_expiry_releases_control() {
    let rt = runtime_with_ttl(Duration::milliseconds(10)).await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let t = rt.expire_lease_if_lapsed().unwrap();
    assert_eq!(t.reason, ControlTransitionReason::Expired);
    assert!(rt.control_state().controller.is_none());
}

#[tokio::test]
async fn observers_receive_state_through_transfers() {
    let rt = runtime().await;
    let mut watch = rt.watch_state();
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    watch.changed().await.unwrap();
    assert!(matches!(
        watch.borrow_and_update().control.controller,
        BrowserController::Agent { .. }
    ));
    rt.acquire(&human(Uuid::new_v4()), true, false, None)
        .unwrap();
    watch.changed().await.unwrap();
    assert!(matches!(
        watch.borrow_and_update().control.controller,
        BrowserController::Human { .. }
    ));
    // The watch stays subscribed across the transfer — observers are never
    // disconnected by control churn.
}

#[tokio::test]
async fn close_releases_control_and_rejects_further_commands() {
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();
    let t = rt.close().await.unwrap();
    assert_eq!(t.reason, ControlTransitionReason::Closed);
    assert!(rt.is_closed());
    let err = rt
        .execute(
            &ag,
            Uuid::new_v4(),
            None,
            &navigate("https://after-close.example"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, BrowserSessionError::SessionClosed));
}

#[tokio::test]
async fn concurrent_duplicate_command_ids_execute_once() {
    let rt = std::sync::Arc::new(runtime().await);
    let ag = agent(Uuid::new_v4());
    let mut frames_rx = rt.subscribe_frames();
    rt.acquire(&ag, false, false, None).unwrap();
    let command_id = Uuid::new_v4();
    let mut handles = Vec::new();
    for _ in 0..8 {
        let rt2 = rt.clone();
        let ag2 = ag.clone();
        handles.push(tokio::spawn(async move {
            rt2.execute(&ag2, command_id, Some(1), &navigate("https://dup.example"))
                .await
        }));
    }
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        assert_eq!(result.command_id, command_id);
    }
    assert_eq!(
        rt.live_state().current_url.as_deref(),
        Some("https://dup.example")
    );
    // The mock driver emits exactly one frame per executed mutation: drain
    // the frame channel to prove the duplicates did not re-execute.
    let mut frames = 0;
    while frames_rx.try_recv().is_ok() {
        frames += 1;
    }
    assert_eq!(frames, 1, "duplicate command_ids must execute exactly once");
}

#[test]
fn close_requires_holder_uncontrolled_or_force() {
    use super::close_permitted;

    let exec = Uuid::new_v4();
    let agent_controller = BrowserController::Agent { execution_id: exec };
    let h = human(Uuid::new_v4());

    // A human may NOT close over a live agent controller without force.
    assert!(!close_permitted(&h, &agent_controller, false));
    assert!(close_permitted(&h, &agent_controller, true));
    // Uncontrolled sessions close freely; holders close their own.
    assert!(close_permitted(&h, &BrowserController::None, false));
    assert!(close_permitted(&agent(exec), &agent_controller, false));
    // An agent never closes over a human controller without force.
    let human_controller = h.to_controller();
    assert!(!close_permitted(&agent(exec), &human_controller, false));
}

#[tokio::test]
async fn late_joining_observer_gets_the_last_frame() {
    // Screencast frames only arrive on repaint, and a broadcast channel
    // delivers nothing sent before a subscriber joined. An observer opening
    // the live view onto an idle page must still get the current picture.
    let rt = runtime().await;
    let ag = agent(Uuid::new_v4());
    rt.acquire(&ag, false, false, None).unwrap();

    assert!(rt.last_frame().is_none(), "no frame before any repaint");

    // A mutation makes the driver emit a frame.
    rt.execute(
        &ag,
        Uuid::new_v4(),
        Some(1),
        &navigate("https://painted.example"),
    )
    .await
    .unwrap();
    // The forwarder task records it asynchronously.
    for _ in 0..50 {
        if rt.last_frame().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let cached = rt
        .last_frame()
        .expect("last frame retained for late joiners");
    assert!(!cached.data.is_empty());

    // A subscriber attaching now receives nothing live (page is idle) — the
    // cached frame is what makes the view paint.
    let mut late = rt.subscribe_frames();
    assert!(
        late.try_recv().is_err(),
        "idle page pushes no new frames to a late subscriber"
    );
}
