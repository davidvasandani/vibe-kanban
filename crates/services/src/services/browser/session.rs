use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, RwLock},
};

use chrono::{DateTime, Duration, Utc};
use tokio::sync::{Mutex as AsyncMutex, broadcast, watch};
use uuid::Uuid;

use super::{
    arbiter::{ControlArbiter, TransferTarget, Transition},
    driver::{DRIVER_OP_TIMEOUT, DriverError, DriverHandle},
    types::{
        BrowserAction, BrowserActionResult, BrowserControlState, BrowserController, BrowserFrame,
        BrowserPageInfo, BrowserSessionError, BrowserSessionLiveState, BrowserSessionStatus,
        ControlPrincipal, ControlTransitionReason,
    },
};

const IDEMPOTENCY_CAPACITY: usize = 256;
const FRAME_FANOUT_CAPACITY: usize = 32;

type IdemOutcome = Result<BrowserActionResult, BrowserSessionError>;

/// Executed-command cache with in-flight reservation: the first caller for a
/// `command_id` becomes the owner and executes; concurrent duplicates wait on
/// the owner's outcome; later duplicates get the recorded result. Only
/// commands that actually reached the driver are recorded durably — control
/// rejections are not "executions" and a legitimate retry under a fresh
/// generation must be allowed to run.
struct IdempotencyCache {
    entries: HashMap<Uuid, IdemEntry>,
    order: VecDeque<Uuid>,
}

enum IdemEntry {
    InFlight(watch::Receiver<Option<IdemOutcome>>),
    Done(IdemOutcome),
}

enum IdemBegin {
    Owner(watch::Sender<Option<IdemOutcome>>),
    Wait(watch::Receiver<Option<IdemOutcome>>),
    Done(IdemOutcome),
}

impl IdempotencyCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn record_done(&mut self, id: Uuid, outcome: IdemOutcome) {
        self.entries.insert(id, IdemEntry::Done(outcome));
        if !self.order.contains(&id) {
            self.order.push_back(id);
        }
        while self.order.len() > IDEMPOTENCY_CAPACITY {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            }
        }
    }
}

/// Removes an in-flight reservation if the owning request is dropped
/// (client disconnect mid-command) so the command_id is not poisoned.
struct IdemOwnerGuard {
    cache: Arc<Mutex<IdempotencyCache>>,
    id: Uuid,
    finished: bool,
}

impl Drop for IdemOwnerGuard {
    fn drop(&mut self) {
        if !self.finished
            && let Ok(mut cache) = self.cache.lock()
            && matches!(cache.entries.get(&self.id), Some(IdemEntry::InFlight(_)))
        {
            cache.entries.remove(&self.id);
        }
    }
}

/// Host-authoritative runtime state for one managed browser session.
///
/// Concurrency model (the command gateway):
/// - `control` (std mutex, never held across await) owns the arbiter; all
///   check-then-act control logic is atomic under it.
/// - `command_gate` (tokio mutex) serializes mutating commands. A command
///   records its admitted generation *before* waiting for the gate and
///   re-checks it *after* acquiring it — a takeover that happened in between
///   turns the queued command into a typed CONTROL_LOST without executing it.
/// - Read-only operations (`screenshot`, `page_info`, frame subscription,
///   state watch) clone the driver handle without touching the command gate,
///   so observers stay live while a controller runs long mutations.
pub struct SessionRuntime {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub profile: Option<String>,
    control: Mutex<ControlArbiter>,
    handle: RwLock<Option<Arc<dyn DriverHandle>>>,
    command_gate: AsyncMutex<()>,
    idempotency: Arc<Mutex<IdempotencyCache>>,
    state_tx: watch::Sender<BrowserSessionLiveState>,
    frames_tx: broadcast::Sender<BrowserFrame>,
    last_frame: Arc<Mutex<Option<BrowserFrame>>>,
    frame_forwarder: Mutex<Option<tokio::task::JoinHandle<()>>>,
    last_activity: Mutex<DateTime<Utc>>,
    idle_expiry: Duration,
}

impl SessionRuntime {
    pub fn new(
        id: Uuid,
        workspace_id: Uuid,
        profile: Option<String>,
        handle: Box<dyn DriverHandle>,
        lease_ttl: Duration,
        idle_expiry: Duration,
    ) -> Self {
        let now = Utc::now();
        let arbiter = ControlArbiter::new(lease_ttl);
        let initial = BrowserSessionLiveState {
            session_id: id,
            workspace_id,
            status: BrowserSessionStatus::Running,
            current_url: None,
            page_title: None,
            control: arbiter.state(),
            expires_at: Some(now + idle_expiry),
        };
        let (state_tx, _) = watch::channel(initial);
        let (frames_tx, _) = broadcast::channel(FRAME_FANOUT_CAPACITY);
        let mut driver_frames = handle.subscribe_frames();
        let forwarder_tx = frames_tx.clone();
        // Retain the most recent frame: CDP only emits on repaint, and a
        // broadcast channel delivers nothing sent before a subscriber joined.
        // Without this, anyone opening the live view onto an idle page waits
        // indefinitely for a frame that only arrives on the next repaint.
        let last_frame = Arc::new(Mutex::new(None::<BrowserFrame>));
        let forwarder_last = last_frame.clone();
        let frame_forwarder = tokio::spawn(async move {
            loop {
                match driver_frames.recv().await {
                    Ok(frame) => {
                        *forwarder_last.lock().unwrap() = Some(frame.clone());
                        let _ = forwarder_tx.send(frame);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        Self {
            id,
            workspace_id,
            profile,
            control: Mutex::new(arbiter),
            handle: RwLock::new(Some(Arc::from(handle))),
            command_gate: AsyncMutex::new(()),
            idempotency: Arc::new(Mutex::new(IdempotencyCache::new())),
            state_tx,
            frames_tx,
            last_frame,
            frame_forwarder: Mutex::new(Some(frame_forwarder)),
            last_activity: Mutex::new(now),
            idle_expiry,
        }
    }

    pub fn live_state(&self) -> BrowserSessionLiveState {
        self.state_tx.borrow().clone()
    }

    pub fn watch_state(&self) -> watch::Receiver<BrowserSessionLiveState> {
        self.state_tx.subscribe()
    }

    pub fn subscribe_frames(&self) -> broadcast::Receiver<BrowserFrame> {
        self.frames_tx.subscribe()
    }

    /// Most recent screencast frame, if any. New observers are sent this
    /// immediately so the live view paints at once instead of waiting for
    /// the page's next repaint.
    pub fn last_frame(&self) -> Option<BrowserFrame> {
        self.last_frame.lock().unwrap().clone()
    }

    pub fn control_state(&self) -> BrowserControlState {
        let mut control = self.control.lock().unwrap();
        // Lazy expiry so observers never see a lapsed lease as live.
        let expired = control.expire_if_lapsed(Utc::now());
        let state = control.state();
        drop(control);
        if expired.is_some() {
            self.publish_control(state.clone());
        }
        state
    }

    pub fn is_closed(&self) -> bool {
        self.state_tx.borrow().status == BrowserSessionStatus::Closed
    }

    fn touch(&self) {
        let now = Utc::now();
        *self.last_activity.lock().unwrap() = now;
        let expires = now + self.idle_expiry;
        self.state_tx.send_if_modified(|state| {
            state.expires_at = Some(expires);
            false // expiry drift alone is not worth waking observers
        });
    }

    pub fn idle_deadline(&self) -> DateTime<Utc> {
        *self.last_activity.lock().unwrap() + self.idle_expiry
    }

    fn publish_control(&self, control: BrowserControlState) {
        self.state_tx.send_if_modified(|state| {
            let changed = state.control.generation != control.generation
                || state.control.lease_expires_at != control.lease_expires_at
                || state.control.controller != control.controller;
            if changed {
                state.control = control.clone();
            }
            changed
        });
    }

    fn ensure_open(&self) -> Result<(), BrowserSessionError> {
        if self.is_closed() {
            Err(BrowserSessionError::SessionClosed)
        } else {
            Ok(())
        }
    }

    // ── Control operations ──────────────────────────────────────────────

    pub fn acquire(
        &self,
        principal: &ControlPrincipal,
        take_from_agent: bool,
        force: bool,
        expected_generation: Option<u64>,
    ) -> Result<Transition, BrowserSessionError> {
        self.ensure_open()?;
        let mut control = self.control.lock().unwrap();
        let transition = control.acquire(
            principal,
            take_from_agent,
            force,
            expected_generation,
            Utc::now(),
        )?;
        let state = control.state();
        drop(control);
        self.publish_control(state);
        self.touch();
        Ok(transition)
    }

    pub fn release(
        &self,
        principal: &ControlPrincipal,
        expected_generation: Option<u64>,
    ) -> Result<Transition, BrowserSessionError> {
        self.ensure_open()?;
        let mut control = self.control.lock().unwrap();
        let transition = control.release(principal, expected_generation, Utc::now())?;
        let state = control.state();
        drop(control);
        self.publish_control(state);
        Ok(transition)
    }

    pub fn transfer(
        &self,
        principal: &ControlPrincipal,
        target: TransferTarget,
        expected_generation: u64,
    ) -> Result<Transition, BrowserSessionError> {
        self.ensure_open()?;
        let mut control = self.control.lock().unwrap();
        let transition = control.transfer(principal, target, expected_generation, Utc::now())?;
        let state = control.state();
        drop(control);
        self.publish_control(state);
        self.touch();
        Ok(transition)
    }

    /// System-initiated release (disconnect, execution completion, expiry
    /// sweep). Returns the transition if the predicate matched the holder.
    pub fn release_if(
        &self,
        predicate: impl FnOnce(&BrowserController) -> bool,
        reason: ControlTransitionReason,
    ) -> Option<Transition> {
        let mut control = self.control.lock().unwrap();
        let transition = control.release_if(predicate, reason, Utc::now())?;
        let state = control.state();
        drop(control);
        self.publish_control(state);
        Some(transition)
    }

    /// Expiry sweep entry point: publishes and returns the transition if the
    /// holder's lease lapsed.
    pub fn expire_lease_if_lapsed(&self) -> Option<Transition> {
        let mut control = self.control.lock().unwrap();
        let transition = control.expire_if_lapsed(Utc::now())?;
        let state = control.state();
        drop(control);
        self.publish_control(state);
        Some(transition)
    }

    // ── Mutating command gateway ────────────────────────────────────────

    pub async fn execute(
        &self,
        principal: &ControlPrincipal,
        command_id: Uuid,
        expected_generation: Option<u64>,
        action: &BrowserAction,
    ) -> Result<BrowserActionResult, BrowserSessionError> {
        self.ensure_open()?;
        // Atomic check-and-reserve: exactly one concurrent request per
        // command_id becomes the owner; duplicates wait for its outcome or
        // get the recorded result. This is what makes duplicate submissions
        // safe under concurrency, not just after completion.
        let begin = {
            let mut cache = self.idempotency.lock().unwrap();
            match cache.entries.get(&command_id) {
                Some(IdemEntry::Done(outcome)) => IdemBegin::Done(outcome.clone()),
                Some(IdemEntry::InFlight(rx)) => IdemBegin::Wait(rx.clone()),
                None => {
                    let (tx, rx) = watch::channel(None);
                    cache.entries.insert(command_id, IdemEntry::InFlight(rx));
                    IdemBegin::Owner(tx)
                }
            }
        };
        let outcome_tx = match begin {
            IdemBegin::Done(outcome) => {
                return outcome;
            }
            IdemBegin::Wait(mut rx) => {
                loop {
                    if let Some(outcome) = rx.borrow_and_update().clone() {
                        return outcome;
                    }
                    if rx.changed().await.is_err() {
                        // Owner dropped without finishing (client disconnect);
                        // outcome unknown — surface as retryable timeout.
                        return Err(BrowserSessionError::Timeout);
                    }
                }
            }
            IdemBegin::Owner(tx) => tx,
        };
        let mut owner_guard = IdemOwnerGuard {
            cache: self.idempotency.clone(),
            id: command_id,
            finished: false,
        };

        let finish = |executed: bool, outcome: IdemOutcome| {
            let mut cache = self.idempotency.lock().unwrap();
            if executed {
                cache.record_done(command_id, outcome.clone());
            } else {
                cache.entries.remove(&command_id);
            }
            drop(cache);
            let _ = outcome_tx.send(Some(outcome.clone()));
            outcome
        };

        // Admission: must hold the lease at the expected generation. Renews
        // the lease (activity from the holder).
        let admission = {
            let mut control = self.control.lock().unwrap();
            let admitted = control.admit_command(principal, expected_generation, Utc::now());
            (admitted, control.state())
        };
        let admitted_generation = match admission {
            (Ok(generation), state) => {
                // Renewal moved lease_expires_at; keep observers accurate.
                self.publish_control(state);
                generation
            }
            (Err(e), _) => {
                owner_guard.finished = true;
                return finish(false, Err(e));
            }
        };

        // Queue point: wait for the command gate. A takeover can land while
        // we wait; the re-check below turns us into CONTROL_LOST without
        // touching the browser (invalidate, never replay).
        let _gate = self.command_gate.lock().await;
        if let Err(e) = self
            .control
            .lock()
            .unwrap()
            .recheck_generation(admitted_generation)
        {
            owner_guard.finished = true;
            return finish(false, Err(e));
        }
        let handle = match self.current_handle() {
            Some(handle) => handle,
            None => {
                owner_guard.finished = true;
                return finish(false, Err(BrowserSessionError::SessionClosed));
            }
        };

        let outcome = tokio::time::timeout(DRIVER_OP_TIMEOUT, handle.perform(action)).await;
        let (result, page) = match outcome {
            Err(_) => (Err(BrowserSessionError::Timeout), None),
            Ok(Err(e)) => (Err(map_driver_error(e)), None),
            Ok(Ok(value)) => {
                let page = handle.page_info().await.ok();
                (Ok(value), page)
            }
        };

        self.touch();
        if let Some(page) = &page {
            self.state_tx.send_if_modified(|state| {
                let changed = state.current_url != page.url || state.page_title != page.title;
                state.current_url = page.url.clone();
                state.page_title = page.title.clone();
                changed
            });
        }

        owner_guard.finished = true;
        match result {
            Ok(value) => {
                let action_result = BrowserActionResult {
                    command_id,
                    generation: admitted_generation,
                    value,
                    current_url: page.and_then(|p| p.url),
                };
                finish(true, Ok(action_result))
            }
            Err(err) => {
                // The command reached the driver; record driver-side failures
                // (and ambiguous timeouts) so a duplicate does not
                // double-execute.
                let executed = matches!(
                    err,
                    BrowserSessionError::Driver { .. } | BrowserSessionError::Timeout
                );
                finish(executed, Err(err))
            }
        }
    }

    fn current_handle(&self) -> Option<Arc<dyn DriverHandle>> {
        self.handle.read().unwrap().clone()
    }

    // ── Read-only operations (concurrent, never control-gated) ──────────

    pub async fn screenshot(&self) -> Result<Vec<u8>, BrowserSessionError> {
        self.ensure_open()?;
        let handle = self
            .current_handle()
            .ok_or(BrowserSessionError::SessionClosed)?;
        tokio::time::timeout(DRIVER_OP_TIMEOUT, handle.screenshot())
            .await
            .map_err(|_| BrowserSessionError::Timeout)?
            .map_err(map_driver_error)
    }

    pub async fn page_info(&self) -> Result<BrowserPageInfo, BrowserSessionError> {
        self.ensure_open()?;
        let handle = self
            .current_handle()
            .ok_or(BrowserSessionError::SessionClosed)?;
        tokio::time::timeout(DRIVER_OP_TIMEOUT, handle.page_info())
            .await
            .map_err(|_| BrowserSessionError::Timeout)?
            .map_err(map_driver_error)
    }

    // ── Lifecycle ───────────────────────────────────────────────────────

    /// Close the session: releases control (reason Closed), shuts the driver
    /// down, and marks the live state Closed. Idempotent.
    pub async fn close(&self) -> Option<Transition> {
        let transition = self.release_if(|_| true, ControlTransitionReason::Closed);
        // Let any in-flight mutation reach its boundary before shutdown.
        let _gate = self.command_gate.lock().await;
        let handle = self.handle.write().unwrap().take();
        if let Some(handle) = handle {
            handle.close().await;
        }
        if let Some(forwarder) = self.frame_forwarder.lock().unwrap().take() {
            forwarder.abort();
        }
        self.state_tx.send_if_modified(|state| {
            if state.status == BrowserSessionStatus::Closed {
                return false;
            }
            state.status = BrowserSessionStatus::Closed;
            true
        });
        transition
    }
}

fn map_driver_error(e: DriverError) -> BrowserSessionError {
    match e {
        DriverError::Unavailable(message) => BrowserSessionError::BrowserUnavailable { message },
        DriverError::Timeout => BrowserSessionError::Timeout,
        DriverError::Gone => BrowserSessionError::SessionClosed,
        DriverError::Protocol(message) => BrowserSessionError::Driver { message },
    }
}
