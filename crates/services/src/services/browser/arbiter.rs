use chrono::{DateTime, Duration, Utc};

use super::types::{
    BrowserControlState, BrowserController, BrowserSessionError, ControlPrincipal,
    ControlTransitionReason,
};

/// Pure control-lease state machine for one browser session.
///
/// All methods are synchronous and are called with the owning session's
/// control lock held, so every check-then-act here is atomic
/// (compare-and-swap reduces to a plain compare). The generation is strictly
/// monotonic: every successful transition increments it, which is what
/// invalidates queued commands admitted under an older generation.
#[derive(Debug)]
pub struct ControlArbiter {
    controller: BrowserController,
    generation: u64,
    lease_expires_at: Option<DateTime<Utc>>,
    lease_ttl: Duration,
}

/// Outcome of a successful transition, for audit/broadcast by the caller.
#[derive(Debug, Clone)]
pub struct Transition {
    pub controller: BrowserController,
    pub generation: u64,
    pub reason: ControlTransitionReason,
}

/// Target of an explicit transfer.
#[derive(Debug, Clone)]
pub enum TransferTarget {
    None,
    Agent { execution_id: uuid::Uuid },
}

impl ControlArbiter {
    pub fn new(lease_ttl: Duration) -> Self {
        Self {
            controller: BrowserController::None,
            generation: 0,
            lease_expires_at: None,
            lease_ttl,
        }
    }

    /// Lazily expire the lease. Returns a transition if the holder's lease
    /// lapsed (callers must audit/broadcast it).
    pub fn expire_if_lapsed(&mut self, now: DateTime<Utc>) -> Option<Transition> {
        if self.controller.is_none() {
            return None;
        }
        match self.lease_expires_at {
            Some(deadline) if deadline <= now => Some(self.transition(
                BrowserController::None,
                ControlTransitionReason::Expired,
                now,
            )),
            _ => None,
        }
    }

    pub fn state(&self) -> BrowserControlState {
        BrowserControlState {
            controller: self.controller.clone(),
            generation: self.generation,
            lease_expires_at: self.lease_expires_at,
        }
    }

    pub fn controller(&self) -> &BrowserController {
        &self.controller
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn conflict(&self) -> BrowserSessionError {
        BrowserSessionError::ControlConflict {
            controller: self.controller.clone(),
            generation: self.generation,
        }
    }

    fn stale(&self) -> BrowserSessionError {
        BrowserSessionError::StaleGeneration {
            controller: self.controller.clone(),
            generation: self.generation,
        }
    }

    fn transition(
        &mut self,
        controller: BrowserController,
        reason: ControlTransitionReason,
        now: DateTime<Utc>,
    ) -> Transition {
        self.generation += 1;
        self.lease_expires_at = if controller.is_none() {
            None
        } else {
            Some(now + self.lease_ttl)
        };
        self.controller = controller;
        Transition {
            controller: self.controller.clone(),
            generation: self.generation,
            reason,
        }
    }

    /// Acquire the lease for `principal`.
    ///
    /// - Uncontrolled (or lapsed) sessions can be acquired by anyone.
    /// - A human with `take_from_agent` displaces an agent lease (takeover).
    /// - `force` (privileged, human paths only) displaces any controller.
    /// - Agents never displace a live controller.
    /// - If the caller already holds control, the lease is renewed without a
    ///   generation bump (idempotent acquire).
    pub fn acquire(
        &mut self,
        principal: &ControlPrincipal,
        take_from_agent: bool,
        force: bool,
        expected_generation: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<Transition, BrowserSessionError> {
        let _ = self.expire_if_lapsed(now);
        if let Some(expected) = expected_generation
            && expected != self.generation
        {
            return Err(self.stale());
        }
        if principal.matches(&self.controller) {
            self.lease_expires_at = Some(now + self.lease_ttl);
            return Ok(Transition {
                controller: self.controller.clone(),
                generation: self.generation,
                reason: ControlTransitionReason::Acquire,
            });
        }
        let (allowed, reason) = match (&self.controller, principal) {
            (BrowserController::None, _) => (true, ControlTransitionReason::Acquire),
            (BrowserController::Agent { .. }, ControlPrincipal::Human { .. })
                if take_from_agent || force =>
            {
                (true, ControlTransitionReason::Takeover)
            }
            (_, ControlPrincipal::Human { .. }) if force => {
                (true, ControlTransitionReason::Takeover)
            }
            _ => (false, ControlTransitionReason::Acquire),
        };
        if !allowed {
            return Err(self.conflict());
        }
        Ok(self.transition(principal.to_controller(), reason, now))
    }

    /// Release the lease. Only the holder may release (no force here; forced
    /// displacement goes through `acquire` so it is always audited as a
    /// takeover).
    pub fn release(
        &mut self,
        principal: &ControlPrincipal,
        expected_generation: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<Transition, BrowserSessionError> {
        let _ = self.expire_if_lapsed(now);
        if let Some(expected) = expected_generation
            && expected != self.generation
        {
            return Err(self.stale());
        }
        if self.controller.is_none() {
            // Releasing an uncontrolled session is a no-op success so
            // release is idempotent for callers cleaning up.
            return Ok(Transition {
                controller: self.controller.clone(),
                generation: self.generation,
                reason: ControlTransitionReason::Release,
            });
        }
        if !principal.matches(&self.controller) {
            return Err(self.conflict());
        }
        Ok(self.transition(
            BrowserController::None,
            ControlTransitionReason::Release,
            now,
        ))
    }

    /// Transfer the lease to `target`. Holder-only, CAS on
    /// `expected_generation` (mandatory).
    pub fn transfer(
        &mut self,
        principal: &ControlPrincipal,
        target: TransferTarget,
        expected_generation: u64,
        now: DateTime<Utc>,
    ) -> Result<Transition, BrowserSessionError> {
        let _ = self.expire_if_lapsed(now);
        if expected_generation != self.generation {
            return Err(self.stale());
        }
        if self.controller.is_none() {
            return Err(self.conflict());
        }
        if !principal.matches(&self.controller) {
            return Err(self.conflict());
        }
        let next = match target {
            TransferTarget::None => BrowserController::None,
            TransferTarget::Agent { execution_id } => BrowserController::Agent { execution_id },
        };
        Ok(self.transition(next, ControlTransitionReason::Transfer, now))
    }

    /// System-initiated release (expiry sweep, WS disconnect, execution
    /// completion, session close). `predicate` decides whether the current
    /// controller is affected.
    pub fn release_if(
        &mut self,
        predicate: impl FnOnce(&BrowserController) -> bool,
        reason: ControlTransitionReason,
        now: DateTime<Utc>,
    ) -> Option<Transition> {
        if self.controller.is_none() {
            return None;
        }
        if predicate(&self.controller) {
            Some(self.transition(BrowserController::None, reason, now))
        } else {
            None
        }
    }

    /// Admission check for a mutating command: the principal must hold the
    /// lease and (when supplied) the expected generation must match. Renews
    /// the lease on success and returns the admitted generation.
    pub fn admit_command(
        &mut self,
        principal: &ControlPrincipal,
        expected_generation: Option<u64>,
        now: DateTime<Utc>,
    ) -> Result<u64, BrowserSessionError> {
        let _ = self.expire_if_lapsed(now);
        if let Some(expected) = expected_generation
            && expected != self.generation
        {
            return Err(self.stale());
        }
        if !principal.matches(&self.controller) {
            return Err(self.conflict());
        }
        self.lease_expires_at = Some(now + self.lease_ttl);
        Ok(self.generation)
    }

    /// Post-queue re-check: a command admitted at `admitted_generation` may
    /// only execute if the generation is unchanged. This is what invalidates
    /// commands that were waiting behind the driver lock across a takeover.
    pub fn recheck_generation(&self, admitted_generation: u64) -> Result<(), BrowserSessionError> {
        if admitted_generation != self.generation {
            return Err(BrowserSessionError::ControlLost {
                controller: self.controller.clone(),
                generation: self.generation,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn agent(id: Uuid) -> ControlPrincipal {
        ControlPrincipal::Agent { execution_id: id }
    }

    fn human(conn: Uuid) -> ControlPrincipal {
        ControlPrincipal::Human {
            user_id: "user".to_string(),
            connection_id: conn,
        }
    }

    fn arbiter() -> ControlArbiter {
        ControlArbiter::new(Duration::seconds(60))
    }

    #[test]
    fn acquire_uncontrolled_bumps_generation() {
        let mut a = arbiter();
        let now = Utc::now();
        let t = a
            .acquire(&agent(Uuid::new_v4()), false, false, None, now)
            .unwrap();
        assert_eq!(t.generation, 1);
        assert!(matches!(t.controller, BrowserController::Agent { .. }));
    }

    #[test]
    fn competing_acquires_exactly_one_wins() {
        let mut a = arbiter();
        let now = Utc::now();
        let first = agent(Uuid::new_v4());
        let second = agent(Uuid::new_v4());
        a.acquire(&first, false, false, None, now).unwrap();
        let err = a.acquire(&second, false, false, None, now).unwrap_err();
        match err {
            BrowserSessionError::ControlConflict { generation, .. } => {
                assert_eq!(generation, 1)
            }
            other => panic!("expected conflict, got {other:?}"),
        }
    }

    #[test]
    fn agent_never_displaces_human() {
        let mut a = arbiter();
        let now = Utc::now();
        a.acquire(&human(Uuid::new_v4()), false, false, None, now)
            .unwrap();
        let err = a
            .acquire(&agent(Uuid::new_v4()), false, false, None, now)
            .unwrap_err();
        assert!(matches!(err, BrowserSessionError::ControlConflict { .. }));
    }

    #[test]
    fn human_takeover_from_agent() {
        let mut a = arbiter();
        let now = Utc::now();
        a.acquire(&agent(Uuid::new_v4()), false, false, None, now)
            .unwrap();
        let t = a
            .acquire(&human(Uuid::new_v4()), true, false, None, now)
            .unwrap();
        assert_eq!(t.reason, ControlTransitionReason::Takeover);
        assert_eq!(t.generation, 2);
    }

    #[test]
    fn human_without_takeover_flag_conflicts_with_agent() {
        let mut a = arbiter();
        let now = Utc::now();
        a.acquire(&agent(Uuid::new_v4()), false, false, None, now)
            .unwrap();
        let err = a
            .acquire(&human(Uuid::new_v4()), false, false, None, now)
            .unwrap_err();
        assert!(matches!(err, BrowserSessionError::ControlConflict { .. }));
    }

    #[test]
    fn stale_expected_generation_rejected() {
        let mut a = arbiter();
        let now = Utc::now();
        let h = human(Uuid::new_v4());
        a.acquire(&h, false, false, None, now).unwrap();
        let err = a.transfer(&h, TransferTarget::None, 0, now).unwrap_err();
        assert!(matches!(err, BrowserSessionError::StaleGeneration { .. }));
    }

    #[test]
    fn transfer_to_execution_cas() {
        let mut a = arbiter();
        let now = Utc::now();
        let h = human(Uuid::new_v4());
        a.acquire(&h, false, false, None, now).unwrap();
        let exec = Uuid::new_v4();
        let t = a
            .transfer(&h, TransferTarget::Agent { execution_id: exec }, 1, now)
            .unwrap();
        assert_eq!(t.generation, 2);
        assert_eq!(t.controller.execution_id(), Some(exec));
    }

    #[test]
    fn non_holder_cannot_transfer_or_release() {
        let mut a = arbiter();
        let now = Utc::now();
        let h = human(Uuid::new_v4());
        a.acquire(&h, false, false, None, now).unwrap();
        let intruder = human(Uuid::new_v4());
        assert!(matches!(
            a.transfer(&intruder, TransferTarget::None, 1, now),
            Err(BrowserSessionError::ControlConflict { .. })
        ));
        assert!(matches!(
            a.release(&intruder, None, now),
            Err(BrowserSessionError::ControlConflict { .. })
        ));
    }

    #[test]
    fn lease_expiry_frees_control() {
        let mut a = ControlArbiter::new(Duration::seconds(1));
        let now = Utc::now();
        let ag = agent(Uuid::new_v4());
        a.acquire(&ag, false, false, None, now).unwrap();
        let later = now + Duration::seconds(2);
        let other = agent(Uuid::new_v4());
        let t = a.acquire(&other, false, false, None, later).unwrap();
        // Expiry transition (gen 2) then acquire (gen 3).
        assert_eq!(t.generation, 3);
    }

    #[test]
    fn admit_renews_lease_and_recheck_detects_takeover() {
        let mut a = arbiter();
        let now = Utc::now();
        let exec = Uuid::new_v4();
        let ag = agent(exec);
        a.acquire(&ag, false, false, None, now).unwrap();
        let admitted = a.admit_command(&ag, Some(1), now).unwrap();
        assert_eq!(admitted, 1);
        // Human takeover happens while the command waits for the driver.
        a.acquire(&human(Uuid::new_v4()), true, false, None, now)
            .unwrap();
        let err = a.recheck_generation(admitted).unwrap_err();
        match err {
            BrowserSessionError::ControlLost { generation, .. } => assert_eq!(generation, 2),
            other => panic!("expected CONTROL_LOST, got {other:?}"),
        }
    }

    #[test]
    fn idempotent_reacquire_by_holder_keeps_generation() {
        let mut a = arbiter();
        let now = Utc::now();
        let exec = Uuid::new_v4();
        let ag = agent(exec);
        a.acquire(&ag, false, false, None, now).unwrap();
        let t = a.acquire(&ag, false, false, None, now).unwrap();
        assert_eq!(t.generation, 1);
    }

    #[test]
    fn release_if_matches_execution() {
        let mut a = arbiter();
        let now = Utc::now();
        let exec = Uuid::new_v4();
        a.acquire(&agent(exec), false, false, None, now).unwrap();
        let none = a.release_if(
            |c| c.execution_id() == Some(Uuid::new_v4()),
            ControlTransitionReason::ExecutionCompleted,
            now,
        );
        assert!(none.is_none());
        let t = a
            .release_if(
                |c| c.execution_id() == Some(exec),
                ControlTransitionReason::ExecutionCompleted,
                now,
            )
            .unwrap();
        assert_eq!(t.reason, ControlTransitionReason::ExecutionCompleted);
        assert!(t.controller.is_none());
    }
}
