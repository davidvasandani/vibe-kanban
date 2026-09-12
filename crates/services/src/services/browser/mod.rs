pub mod arbiter;
pub mod cdp;
pub mod driver;
pub mod session;
pub mod types;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use chrono::{Duration, Utc};
use dashmap::DashMap;
use db::{
    DBService,
    models::{
        browser_session::{BrowserControlTransition, BrowserSession, BrowserSessionDbStatus},
        execution_process::{ExecutionProcess, ExecutionProcessRunReason},
        session::Session,
    },
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};
use ts_rs::TS;
use uuid::Uuid;

use self::{
    arbiter::{TransferTarget, Transition},
    driver::{BrowserDriver, LaunchOpts},
    session::SessionRuntime,
    types::{
        BrowserAction, BrowserActionResult, BrowserControlState, BrowserController, BrowserFrame,
        BrowserPageInfo, BrowserSessionError, BrowserSessionLiveState, ControlPrincipal,
        ControlTransitionReason,
    },
};

const SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);

/// Select the browser driver for this process:
/// - `VK_BROWSER_MOCK=1` → mock driver (dev/tests; no real browser).
/// - a discoverable Chromium binary → CDP driver.
/// - otherwise → a driver that fails session creation with a typed
///   BROWSER_UNAVAILABLE (the feature degrades cleanly, nothing else breaks).
pub fn select_driver(config: &BrowserSessionsConfig) -> Arc<dyn BrowserDriver> {
    if std::env::var("VK_BROWSER_MOCK").is_ok_and(|v| v == "1") {
        tracing::info!("browser sessions using mock driver (VK_BROWSER_MOCK=1)");
        return Arc::new(driver::MockDriver);
    }
    match cdp::discover_chromium(config.chromium_path.as_deref()) {
        Some(path) => Arc::new(cdp::CdpDriver::new(path)),
        None => Arc::new(driver::UnavailableDriver {
            message: "no Chromium/Chrome binary found; set browser.chromium_path or VIBE_BROWSER_CHROMIUM_PATH".to_string(),
        }),
    }
}

/// Whether `principal` may close a session under `controller`: the holder,
/// an uncontrolled session, or an explicit force. A live controller (human
/// or agent) is never displaced by a plain close.
pub fn close_permitted(
    principal: &ControlPrincipal,
    controller: &BrowserController,
    force: bool,
) -> bool {
    force || controller.is_none() || principal.matches(controller)
}

#[derive(Debug, Clone)]
pub struct BrowserSessionsConfig {
    pub enabled: bool,
    pub allow_evaluate: bool,
    pub lease_ttl: Duration,
    pub idle_expiry: Duration,
    pub chromium_path: Option<String>,
}

impl Default for BrowserSessionsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allow_evaluate: true,
            lease_ttl: Duration::seconds(60),
            idle_expiry: Duration::minutes(120),
            chromium_path: None,
        }
    }
}

/// A session row combined with its live, host-authoritative state (when the
/// session is open on this host).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BrowserSessionWithState {
    pub session: BrowserSession,
    pub live: Option<BrowserSessionLiveState>,
}

/// Workspace-scoped managed browser sessions with a single per-session
/// control arbiter. Every mutating path (REST, live-view WS, MCP via REST)
/// resolves to the methods on this service; live control state is
/// authoritative here (in memory, on the host owning Chromium) and the
/// database records sessions + transitions for audit only.
#[derive(Clone)]
pub struct BrowserSessionService {
    inner: Arc<Inner>,
}

struct Inner {
    db: DBService,
    driver: Arc<dyn BrowserDriver>,
    config: BrowserSessionsConfig,
    host_id: String,
    sessions: DashMap<Uuid, Arc<SessionRuntime>>,
}

impl std::fmt::Debug for BrowserSessionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSessionService")
            .field("host_id", &self.inner.host_id)
            .field("open_sessions", &self.inner.sessions.len())
            .finish()
    }
}

impl BrowserSessionService {
    pub fn new(
        db: DBService,
        driver: Arc<dyn BrowserDriver>,
        config: BrowserSessionsConfig,
        host_id: String,
    ) -> Self {
        let service = Self {
            inner: Arc::new(Inner {
                db,
                driver,
                config,
                host_id,
                sessions: DashMap::new(),
            }),
        };
        service.spawn_sweeper();
        service
    }

    pub fn config(&self) -> &BrowserSessionsConfig {
        &self.inner.config
    }

    fn runtime(&self, id: Uuid) -> Result<Arc<SessionRuntime>, BrowserSessionError> {
        self.inner
            .sessions
            .get(&id)
            .map(|r| r.value().clone())
            .ok_or(BrowserSessionError::NotFound)
    }

    // ── Lifecycle ───────────────────────────────────────────────────────

    pub async fn create_session(
        &self,
        workspace_id: Uuid,
        profile: Option<String>,
    ) -> Result<BrowserSessionWithState, BrowserSessionError> {
        if !self.inner.config.enabled {
            return Err(BrowserSessionError::BrowserUnavailable {
                message: "browser sessions are disabled by configuration".to_string(),
            });
        }
        let handle = self
            .inner
            .driver
            .launch(LaunchOpts {
                profile: profile.clone(),
                viewport: None,
            })
            .await
            .map_err(|e| match e {
                driver::DriverError::Unavailable(message) => {
                    BrowserSessionError::BrowserUnavailable { message }
                }
                other => BrowserSessionError::Driver {
                    message: other.to_string(),
                },
            })?;
        let pgid = handle.pgid();
        let id = Uuid::new_v4();
        let runtime = Arc::new(SessionRuntime::new(
            id,
            workspace_id,
            profile.clone(),
            handle,
            self.inner.config.lease_ttl,
            self.inner.config.idle_expiry,
        ));
        // Register the runtime before persisting so a concurrent lookup never
        // finds a DB row without its live owner.
        self.inner.sessions.insert(id, runtime.clone());
        let row = BrowserSession::create(
            &self.inner.db.pool,
            id,
            workspace_id,
            &self.inner.host_id,
            profile.as_deref(),
            BrowserSessionDbStatus::Running,
            Some(Utc::now() + self.inner.config.idle_expiry),
        )
        .await
        .inspect_err(|_| {
            // Roll back the runtime registration; the Chromium instance is
            // closed asynchronously.
            if let Some((_, runtime)) = self.inner.sessions.remove(&id) {
                tokio::spawn(async move {
                    runtime.close().await;
                });
            }
        })?;
        // Record the process group id so a later boot can clean up the
        // group if this server dies uncleanly. Best-effort: a failure here
        // just means the orphan reaper skips this session.
        if let Some(pgid) = pgid
            && let Err(e) = BrowserSession::update_pgid(&self.inner.db.pool, id, pgid as i64).await
        {
            tracing::warn!(session_id = %id, error = %e, "failed to record pgid for browser session");
        }
        Ok(BrowserSessionWithState {
            session: row,
            live: Some(runtime.live_state()),
        })
    }

    pub async fn get_session(
        &self,
        id: Uuid,
    ) -> Result<BrowserSessionWithState, BrowserSessionError> {
        let row = BrowserSession::find_by_id(&self.inner.db.pool, id)
            .await?
            .ok_or(BrowserSessionError::NotFound)?;
        let live = self.inner.sessions.get(&id).map(|r| r.value().live_state());
        Ok(BrowserSessionWithState { session: row, live })
    }

    pub async fn list_sessions(
        &self,
        workspace_id: Uuid,
        include_closed: bool,
    ) -> Result<Vec<BrowserSessionWithState>, BrowserSessionError> {
        let rows =
            BrowserSession::find_by_workspace(&self.inner.db.pool, workspace_id, include_closed)
                .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let live = self
                    .inner
                    .sessions
                    .get(&row.id)
                    .map(|r| r.value().live_state());
                BrowserSessionWithState { session: row, live }
            })
            .collect())
    }

    /// Close a session. Closing requires holding control, an uncontrolled
    /// session, or an explicit `force` — a live controller (human or agent)
    /// is never displaced silently by a plain close.
    pub async fn close_session(
        &self,
        id: Uuid,
        principal: &ControlPrincipal,
        force: bool,
    ) -> Result<(), BrowserSessionError> {
        let runtime = self.runtime(id)?;
        self.expire_and_audit(&runtime).await;
        {
            let control = runtime.control_state();
            if !close_permitted(principal, &control.controller, force) {
                return Err(BrowserSessionError::ControlConflict {
                    controller: control.controller,
                    generation: control.generation,
                });
            }
        }
        self.close_runtime(&runtime, BrowserSessionDbStatus::Closed)
            .await;
        Ok(())
    }

    async fn close_runtime(&self, runtime: &Arc<SessionRuntime>, status: BrowserSessionDbStatus) {
        let transition = runtime.close().await;
        // Remove from the registry only after the runtime is marked closed so
        // there is no window where the session is in neither owner.
        self.inner.sessions.remove(&runtime.id);
        if let Some(transition) = transition {
            self.audit(runtime.id, &transition).await;
        }
        if let Err(e) = BrowserSession::update_status(&self.inner.db.pool, runtime.id, status).await
        {
            tracing::warn!(session_id = %runtime.id, error = %e, "failed to persist browser session close");
        }
    }

    /// Close every open session belonging to a workspace (archive/delete
    /// cleanup policy).
    pub async fn close_for_workspace(&self, workspace_id: Uuid) {
        let targets: Vec<Arc<SessionRuntime>> = self
            .inner
            .sessions
            .iter()
            .filter(|entry| entry.value().workspace_id == workspace_id)
            .map(|entry| entry.value().clone())
            .collect();
        for runtime in targets {
            self.close_runtime(&runtime, BrowserSessionDbStatus::Closed)
                .await;
        }
    }

    /// Kill Chromium process groups orphaned by an unclean previous-server
    /// exit (crash/SIGKILL) and mark their sessions closed. Call once at
    /// startup, before any new sessions are created — the in-memory
    /// `sessions` registry starts empty on every boot, so rows left
    /// `starting`/`running` from a prior instance have no live owner and
    /// would otherwise never be reaped.
    pub async fn cleanup_orphan_sessions(&self) {
        let open = match BrowserSession::find_open(&self.inner.db.pool).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::warn!(error = %e, "failed to list open browser sessions for orphan cleanup");
                return;
            }
        };
        for session in open {
            #[cfg(unix)]
            if let Some(pgid) = session.pgid {
                let age_secs = (Utc::now() - session.created_at).num_seconds();
                if utils::process::kill_orphan_process_group(pgid as i32, age_secs).await {
                    tracing::info!(
                        session_id = %session.id,
                        pgid,
                        "killed orphaned OS process group for browser session"
                    );
                }
            }
            if let Err(e) = BrowserSession::update_status(
                &self.inner.db.pool,
                session.id,
                BrowserSessionDbStatus::Closed,
            )
            .await
            {
                tracing::warn!(session_id = %session.id, error = %e, "failed to close orphaned browser session");
            }
        }
    }

    // ── Control operations ──────────────────────────────────────────────

    pub async fn get_control(&self, id: Uuid) -> Result<BrowserControlState, BrowserSessionError> {
        let runtime = self.runtime(id)?;
        // Record any lapsed lease before reporting state, so a poll of the
        // control endpoint can never silently swallow the `Expired` transition.
        self.expire_and_audit(&runtime).await;
        Ok(runtime.control_state())
    }

    pub async fn acquire_control(
        &self,
        id: Uuid,
        principal: &ControlPrincipal,
        take_from_agent: bool,
        force: bool,
        expected_generation: Option<u64>,
    ) -> Result<BrowserControlState, BrowserSessionError> {
        let runtime = self.runtime(id)?;
        self.expire_and_audit(&runtime).await;
        let transition = runtime.acquire(principal, take_from_agent, force, expected_generation)?;
        self.audit(id, &transition).await;
        Ok(runtime.control_state())
    }

    pub async fn release_control(
        &self,
        id: Uuid,
        principal: &ControlPrincipal,
        expected_generation: Option<u64>,
    ) -> Result<BrowserControlState, BrowserSessionError> {
        let runtime = self.runtime(id)?;
        self.expire_and_audit(&runtime).await;
        let transition = runtime.release(principal, expected_generation)?;
        self.audit(id, &transition).await;
        Ok(runtime.control_state())
    }

    pub async fn transfer_control(
        &self,
        id: Uuid,
        principal: &ControlPrincipal,
        target: TransferTarget,
        expected_generation: u64,
    ) -> Result<BrowserControlState, BrowserSessionError> {
        let runtime = self.runtime(id)?;
        self.expire_and_audit(&runtime).await;
        if let TransferTarget::Agent { execution_id } = &target {
            self.ensure_execution_in_workspace(*execution_id, runtime.workspace_id)
                .await?;
        }
        let transition = runtime.transfer(principal, target, expected_generation)?;
        self.audit(id, &transition).await;
        Ok(runtime.control_state())
    }

    /// Release any leases held by a completed execution. Sessions stay open.
    pub async fn release_for_execution(&self, execution_id: Uuid) {
        for entry in self.inner.sessions.iter() {
            let runtime = entry.value().clone();
            if let Some(transition) = runtime.release_if(
                |c| c.execution_id() == Some(execution_id),
                ControlTransitionReason::ExecutionCompleted,
            ) {
                self.audit(runtime.id, &transition).await;
            }
        }
    }

    /// Release any human leases bound to a closed live-view connection.
    pub async fn release_for_connection(&self, connection_id: Uuid) {
        for entry in self.inner.sessions.iter() {
            let runtime = entry.value().clone();
            if let Some(transition) = runtime.release_if(
                |c| c.connection_id() == Some(connection_id),
                ControlTransitionReason::Disconnected,
            ) {
                self.audit(runtime.id, &transition).await;
            }
        }
    }

    // ── Agent principal resolution (SPEC §8.3) ──────────────────────────

    /// Resolve the agent principal for a workspace-scoped agent request.
    /// Explicit execution ids are honored only when the execution belongs to
    /// the workspace; otherwise the most recently started running
    /// coding-agent execution is bound.
    pub async fn resolve_agent_principal(
        &self,
        workspace_id: Uuid,
        explicit_execution_id: Option<Uuid>,
    ) -> Result<ControlPrincipal, BrowserSessionError> {
        if let Some(execution_id) = explicit_execution_id {
            self.ensure_execution_in_workspace(execution_id, workspace_id)
                .await?;
            return Ok(ControlPrincipal::Agent { execution_id });
        }
        let running = ExecutionProcess::find_running_by_workspace_and_run_reason(
            &self.inner.db.pool,
            workspace_id,
            &ExecutionProcessRunReason::CodingAgent,
        )
        .await?;
        // Ordered by created_at DESC → most recently started first.
        match running.first() {
            Some(process) => Ok(ControlPrincipal::Agent {
                execution_id: process.id,
            }),
            None => Err(BrowserSessionError::NoRunningExecution),
        }
    }

    async fn ensure_execution_in_workspace(
        &self,
        execution_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(), BrowserSessionError> {
        let process = ExecutionProcess::find_by_id(&self.inner.db.pool, execution_id)
            .await?
            .ok_or(BrowserSessionError::ExecutionNotInWorkspace)?;
        let session = Session::find_by_id(&self.inner.db.pool, process.session_id)
            .await?
            .ok_or(BrowserSessionError::ExecutionNotInWorkspace)?;
        if session.workspace_id != workspace_id {
            return Err(BrowserSessionError::ExecutionNotInWorkspace);
        }
        Ok(())
    }

    // ── Command gateway ─────────────────────────────────────────────────

    /// Execute a mutating action as `principal`. `auto_acquire` (agent action
    /// tools) acquires an *uncontrolled* session first but never displaces a
    /// live controller.
    pub async fn execute_action(
        &self,
        id: Uuid,
        principal: &ControlPrincipal,
        command_id: Uuid,
        expected_generation: Option<u64>,
        action: BrowserAction,
        auto_acquire: bool,
    ) -> Result<BrowserActionResult, BrowserSessionError> {
        if action.is_privileged() && !self.inner.config.allow_evaluate {
            return Err(BrowserSessionError::CapabilityDenied {
                capability: "evaluate".to_string(),
            });
        }
        let runtime = self.runtime(id)?;
        // Audit a lapsed lease before admitting the command; otherwise the
        // auto-acquire probe / command admission would consume the expiry and
        // leave it unrecorded.
        self.expire_and_audit(&runtime).await;
        if auto_acquire {
            let control = runtime.control_state();
            if control.controller.is_none() {
                let transition = runtime.acquire(principal, false, false, None)?;
                self.audit(id, &transition).await;
            }
        }
        let result = runtime
            .execute(principal, command_id, expected_generation, &action)
            .await?;
        let live = runtime.live_state();
        if let Err(e) = BrowserSession::update_activity(
            &self.inner.db.pool,
            id,
            live.current_url.as_deref(),
            live.expires_at,
        )
        .await
        {
            tracing::debug!(session_id = %id, error = %e, "failed to persist browser activity");
        }
        Ok(result)
    }

    // ── Read-only operations ────────────────────────────────────────────

    pub async fn screenshot(&self, id: Uuid) -> Result<Vec<u8>, BrowserSessionError> {
        self.runtime(id)?.screenshot().await
    }

    pub async fn page_info(&self, id: Uuid) -> Result<BrowserPageInfo, BrowserSessionError> {
        self.runtime(id)?.page_info().await
    }

    pub fn subscribe_frames(
        &self,
        id: Uuid,
    ) -> Result<broadcast::Receiver<BrowserFrame>, BrowserSessionError> {
        Ok(self.runtime(id)?.subscribe_frames())
    }

    /// Most recent screencast frame, replayed to newly attached observers.
    pub fn last_frame(&self, id: Uuid) -> Result<Option<BrowserFrame>, BrowserSessionError> {
        Ok(self.runtime(id)?.last_frame())
    }

    pub fn watch_state(
        &self,
        id: Uuid,
    ) -> Result<watch::Receiver<BrowserSessionLiveState>, BrowserSessionError> {
        Ok(self.runtime(id)?.watch_state())
    }

    pub fn live_state(&self, id: Uuid) -> Result<BrowserSessionLiveState, BrowserSessionError> {
        Ok(self.runtime(id)?.live_state())
    }

    // ── Audit + sweeping ────────────────────────────────────────────────

    /// Flush a lapsed lease to the audit log before any observation or
    /// mutation of control state. `expire_if_lapsed` is consume-once: whichever
    /// observer sees the lapse first (a reader like `get_control`, a mutating
    /// op, or the 30s sweeper) clears the lease in memory. The lazy path in
    /// `SessionRuntime::control_state` only *broadcasts* that expiry, so if a
    /// reader observed it first the `Expired` transition was silently dropped
    /// from `browser_control_transitions` (the sweeper then found nothing to
    /// audit). Auditing here — the only control path that also holds the DB
    /// handle — closes that gap. Idempotent: a no-op once the lease is cleared,
    /// so it never double-records with the sweeper.
    async fn expire_and_audit(&self, runtime: &Arc<SessionRuntime>) {
        if let Some(transition) = runtime.expire_lease_if_lapsed() {
            self.audit(runtime.id, &transition).await;
        }
    }

    /// Persist a control transition for audit. URLs/profile contents are
    /// deliberately not part of transition rows (redaction requirement).
    async fn audit(&self, session_id: Uuid, transition: &Transition) {
        let (controller_type, execution_id, user_id, connection_id) = match &transition.controller {
            BrowserController::None => ("none", None, None, None),
            BrowserController::Agent { execution_id } => ("agent", Some(*execution_id), None, None),
            BrowserController::Human {
                user_id,
                connection_id,
            } => ("human", None, Some(user_id.clone()), Some(*connection_id)),
        };
        if let Err(e) = BrowserControlTransition::create(
            &self.inner.db.pool,
            session_id,
            transition.generation as i64,
            controller_type,
            execution_id,
            user_id.as_deref(),
            connection_id,
            transition.reason.as_str(),
        )
        .await
        {
            tracing::warn!(
                session_id = %session_id,
                generation = transition.generation,
                reason = transition.reason.as_str(),
                error = %e,
                "failed to persist browser control transition"
            );
        }
    }

    /// Event-driven cleanup: watch the shared event stream for execution
    /// processes leaving `running` (release their leases) and workspaces
    /// being archived/deleted (close their sessions). Lease TTL expiry is the
    /// backstop if a broadcast event is missed under lag.
    pub fn spawn_cleanup_watcher(&self, msg_store: Arc<utils::msg_store::MsgStore>) {
        use db::models::execution_process::ExecutionProcessStatus;
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            let mut receiver = msg_store.get_receiver();
            loop {
                let msg = match receiver.recv().await {
                    Ok(msg) => msg,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                let Some(inner) = weak.upgrade() else { break };
                let service = BrowserSessionService { inner };
                if service.inner.sessions.is_empty() {
                    continue;
                }
                let utils::log_msg::LogMsg::JsonPatch(patch) = msg else {
                    continue;
                };
                for op in &patch.0 {
                    let path = op.path().to_string();
                    if let Some(exec_segment) = path.strip_prefix("/execution_processes/") {
                        let value = match op {
                            json_patch::PatchOperation::Add(a) => Some(&a.value),
                            json_patch::PatchOperation::Replace(r) => Some(&r.value),
                            json_patch::PatchOperation::Remove(_) => None,
                            _ => None,
                        };
                        if let Some(value) = value {
                            if let Ok(process) = serde_json::from_value::<
                                db::models::execution_process::ExecutionProcess,
                            >(value.clone())
                                && process.status != ExecutionProcessStatus::Running
                            {
                                service.release_for_execution(process.id).await;
                            }
                        } else if let Ok(execution_id) = exec_segment.parse::<Uuid>() {
                            service.release_for_execution(execution_id).await;
                        }
                    } else if let Some(ws_segment) = path.strip_prefix("/workspaces/") {
                        let Ok(workspace_id) = ws_segment.parse::<Uuid>() else {
                            continue;
                        };
                        let archived_or_removed = match op {
                            json_patch::PatchOperation::Remove(_) => true,
                            json_patch::PatchOperation::Add(a) => {
                                a.value.get("archived").and_then(|x| x.as_bool()) == Some(true)
                            }
                            json_patch::PatchOperation::Replace(r) => {
                                r.value.get("archived").and_then(|x| x.as_bool()) == Some(true)
                            }
                            _ => false,
                        };
                        if archived_or_removed {
                            service.close_for_workspace(workspace_id).await;
                        }
                    }
                }
            }
        });
    }

    fn spawn_sweeper(&self) {
        let weak = Arc::downgrade(&self.inner);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(SWEEP_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                // Hold only a weak reference between ticks so dropping the
                // service stops the sweeper instead of leaking it.
                let Some(inner) = weak.upgrade() else { break };
                BrowserSessionService { inner }.sweep_once().await;
            }
        });
    }

    async fn sweep_once(&self) {
        let now = Utc::now();
        let runtimes: Vec<Arc<SessionRuntime>> = self
            .inner
            .sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        for runtime in runtimes {
            if let Some(transition) = runtime.expire_lease_if_lapsed() {
                self.audit(runtime.id, &transition).await;
            }
            // Generation-conditional idle reap: re-check the deadline right
            // before closing so a session that became active in the window is
            // not killed.
            if runtime.idle_deadline() <= now
                && !runtime.is_closed()
                && runtime.idle_deadline() <= Utc::now()
            {
                tracing::info!(session_id = %runtime.id, "closing idle browser session");
                self.close_runtime(&runtime, BrowserSessionDbStatus::Closed)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod action_serde_tests {
    use super::types::BrowserAction;

    // NOTE: this workspace enables serde_json's `preserve_order`, which
    // breaks f64 fields inside internally-tagged enums ("invalid type: map,
    // expected f64"). Browser action coordinates are therefore integers.

    #[test]
    fn action_wire_shapes_parse() {
        for payload in [
            r#"{"type":"navigate","url":"https://x.example"}"#,
            r#"{"type":"back"}"#,
            r#"{"type":"click","x":10,"y":20,"button":"left"}"#,
            r#"{"type":"click","x":10,"y":20}"#,
            r#"{"type":"mouse_move","x":1,"y":2}"#,
            r#"{"type":"set_viewport","width":800,"height":600}"#,
            r#"{"type":"key","key":"Enter"}"#,
            r#"{"type":"type","text":"hello"}"#,
            r#"{"type":"evaluate","expression":"1+1"}"#,
        ] {
            let action: Result<BrowserAction, _> = serde_json::from_str(payload);
            assert!(action.is_ok(), "failed to parse {payload}: {action:?}");
        }
    }
}
