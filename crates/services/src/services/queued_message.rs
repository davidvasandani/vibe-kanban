use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::{DashMap, mapref::entry::Entry};
use db::models::scratch::DraftFollowUpData;
use serde::{Deserialize, Serialize};
use tokio::sync::Notify;
use ts_rs::TS;
use uuid::Uuid;

/// Represents a queued follow-up message for a session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct QueuedMessage {
    /// The session this message is queued for
    pub session_id: Uuid,
    /// The follow-up data (message + variant)
    pub data: DraftFollowUpData,
    /// Timestamp when the message was queued
    pub queued_at: DateTime<Utc>,
    #[serde(default)]
    pub restart_agent: bool,
    #[serde(skip)]
    #[ts(skip)]
    restart_reservation: Option<Uuid>,
    #[serde(skip)]
    #[ts(skip)]
    remove_on_reservation_cancel: bool,
}

/// Status of the queue for a session (for frontend display)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QueueStatus {
    /// No message queued
    Empty,
    /// Message is queued and waiting for execution to complete
    Queued { message: QueuedMessage },
}

/// In-memory service for managing queued follow-up messages.
/// One queued message per session.
#[derive(Clone)]
pub struct QueuedMessageService {
    queue: Arc<DashMap<Uuid, QueuedMessage>>,
    blocked_mcp_restarts: Arc<DashMap<Uuid, ()>>,
    workspace_mcp_restarts: Arc<DashMap<Uuid, DateTime<Utc>>>,
    cancelled_workspace_mcp_restarts: Arc<DashMap<Uuid, DateTime<Utc>>>,
    restart_resolution: Arc<Notify>,
}

impl QueuedMessageService {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(DashMap::new()),
            blocked_mcp_restarts: Arc::new(DashMap::new()),
            workspace_mcp_restarts: Arc::new(DashMap::new()),
            cancelled_workspace_mcp_restarts: Arc::new(DashMap::new()),
            restart_resolution: Arc::new(Notify::new()),
        }
    }

    /// Queue a message for a session. Replaces any existing queued message.
    pub fn queue_message(&self, session_id: Uuid, data: DraftFollowUpData) -> QueuedMessage {
        let queued = QueuedMessage {
            session_id,
            data,
            queued_at: Utc::now(),
            restart_agent: false,
            restart_reservation: None,
            remove_on_reservation_cancel: false,
        };
        self.queue.insert(session_id, queued.clone());
        self.restart_resolution.notify_waiters();
        queued
    }

    pub fn reserve_mcp_restart(&self, session_id: Uuid, data: DraftFollowUpData) -> Uuid {
        let reservation = Uuid::new_v4();
        let queued_at = match self.queue.entry(session_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().restart_reservation = Some(reservation);
                entry.get_mut().remove_on_reservation_cancel = false;
                entry.get().queued_at
            }
            Entry::Vacant(entry) => {
                let queued = QueuedMessage {
                    session_id,
                    data,
                    queued_at: Utc::now(),
                    restart_agent: false,
                    restart_reservation: Some(reservation),
                    remove_on_reservation_cancel: true,
                };
                let queued_at = queued.queued_at;
                entry.insert(queued);
                queued_at
            }
        };
        if self.is_mcp_restart_start_blocked(session_id) {
            self.workspace_mcp_restarts.insert(session_id, queued_at);
        }
        reservation
    }

    pub fn commit_mcp_restart(&self, session_id: Uuid, reservation: Uuid) -> Option<DateTime<Utc>> {
        let result = match self.queue.entry(session_id) {
            Entry::Occupied(mut entry) if entry.get().restart_reservation == Some(reservation) => {
                entry.get_mut().restart_agent = true;
                entry.get_mut().restart_reservation = None;
                Some(entry.get().queued_at)
            }
            _ => None,
        };
        self.restart_resolution.notify_waiters();
        result
    }

    pub fn take_committed_mcp_restart(
        &self,
        session_id: Uuid,
        queued_at: DateTime<Utc>,
    ) -> Option<QueuedMessage> {
        match self.queue.entry(session_id) {
            Entry::Occupied(entry)
                if entry.get().queued_at == queued_at
                    && entry.get().restart_agent
                    && entry.get().restart_reservation.is_none() =>
            {
                Some(entry.remove())
            }
            _ => None,
        }
    }

    pub fn cancel_mcp_restart(&self, session_id: Uuid, reservation: Uuid) {
        if let Entry::Occupied(mut entry) = self.queue.entry(session_id)
            && entry.get().restart_reservation == Some(reservation)
        {
            if entry.get().remove_on_reservation_cancel {
                entry.remove();
            } else {
                entry.get_mut().restart_reservation = None;
                entry.get_mut().remove_on_reservation_cancel = false;
            }
        }
        self.restart_resolution.notify_waiters();
    }

    pub fn take_mcp_restart(&self, session_id: Uuid, reservation: Uuid) -> Option<QueuedMessage> {
        let result = match self.queue.entry(session_id) {
            Entry::Occupied(mut entry) if entry.get().restart_reservation == Some(reservation) => {
                entry.get_mut().restart_agent = true;
                entry.get_mut().restart_reservation = None;
                entry.get_mut().remove_on_reservation_cancel = false;
                Some(entry.remove())
            }
            _ => None,
        };
        self.restart_resolution.notify_waiters();
        result
    }

    /// Cancel/remove a queued message for a session
    pub fn cancel_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        let removed = self.queue.remove(&session_id).map(|(_, v)| v);
        self.restart_resolution.notify_waiters();
        removed
    }

    pub fn supersede_mcp_restart(&self, session_id: Uuid) {
        if let Entry::Occupied(mut entry) = self.queue.entry(session_id) {
            if entry.get().remove_on_reservation_cancel {
                entry.remove();
            } else {
                entry.get_mut().restart_agent = false;
                entry.get_mut().restart_reservation = None;
            }
        }
        self.restart_resolution.notify_waiters();
    }

    /// Get the queued message for a session (if any)
    pub fn get_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        self.queue.get(&session_id).map(|r| r.clone())
    }

    /// Take (remove and return) the queued message for a session.
    /// Used by finalization flow to consume the queued message.
    pub fn take_queued(&self, session_id: Uuid) -> Option<QueuedMessage> {
        match self.queue.entry(session_id) {
            Entry::Occupied(entry) if entry.get().restart_reservation.is_none() => {
                Some(entry.remove())
            }
            _ => None,
        }
    }

    /// Check if a session has a queued message
    pub fn has_queued(&self, session_id: Uuid) -> bool {
        self.queue.contains_key(&session_id)
    }

    pub fn has_pending_restart(&self, session_id: Uuid) -> bool {
        self.queue
            .get(&session_id)
            .is_some_and(|message| message.restart_reservation.is_some())
    }

    pub fn has_mcp_restart(&self, session_id: Uuid) -> bool {
        self.queue
            .get(&session_id)
            .is_some_and(|message| message.restart_agent || message.restart_reservation.is_some())
    }

    pub fn block_mcp_restart_start(&self, session_id: Uuid) {
        self.cancelled_workspace_mcp_restarts.remove(&session_id);
        self.blocked_mcp_restarts.insert(session_id, ());
        if let Some(queued_at) = self.queue.get(&session_id).map(|message| message.queued_at) {
            self.workspace_mcp_restarts.insert(session_id, queued_at);
        }
    }

    pub fn unblock_mcp_restart_start(&self, session_id: Uuid) {
        self.blocked_mcp_restarts.remove(&session_id);
        self.restart_resolution.notify_waiters();
    }

    pub fn is_mcp_restart_start_blocked(&self, session_id: Uuid) -> bool {
        self.blocked_mcp_restarts.contains_key(&session_id)
    }

    pub fn is_workspace_mcp_restart(&self, session_id: Uuid) -> bool {
        self.workspace_mcp_restarts.contains_key(&session_id)
    }

    pub fn has_deferred_mcp_restart(&self, session_id: Uuid) -> bool {
        self.workspace_mcp_restarts.contains_key(&session_id)
            && !self.queue.contains_key(&session_id)
    }

    pub fn finish_workspace_mcp_restart(&self, session_id: Uuid) {
        self.workspace_mcp_restarts.remove(&session_id);
        self.cancelled_workspace_mcp_restarts.remove(&session_id);
    }

    pub fn cancel_workspace_mcp_restart(&self, session_id: Uuid) {
        if let Some((_, queued_at)) = self.workspace_mcp_restarts.remove(&session_id) {
            self.cancelled_workspace_mcp_restarts
                .insert(session_id, queued_at);
        }
        self.unblock_mcp_restart_start(session_id);
    }

    /// Cancel a continuation that has already been claimed by finalization,
    /// without releasing the workspace teardown gate that still owns it.
    pub fn cancel_deferred_mcp_restart(&self, session_id: Uuid) -> bool {
        if let Some(queued_at) = self
            .workspace_mcp_restarts
            .get(&session_id)
            .map(|entry| *entry)
        {
            self.cancelled_workspace_mcp_restarts
                .insert(session_id, queued_at);
            self.restart_resolution.notify_waiters();
            true
        } else {
            false
        }
    }

    pub async fn wait_for_mcp_restart_start(
        &self,
        session_id: Uuid,
        queued_at: DateTime<Utc>,
    ) -> bool {
        while self.is_mcp_restart_start_blocked(session_id) {
            let notified = self.restart_resolution.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.is_mcp_restart_start_blocked(session_id) {
                break;
            }
            notified.await;
        }
        if self
            .cancelled_workspace_mcp_restarts
            .get(&session_id)
            .is_some_and(|cancelled_at| *cancelled_at == queued_at)
        {
            self.cancelled_workspace_mcp_restarts.remove(&session_id);
            false
        } else {
            true
        }
    }

    pub async fn wait_for_restart_resolution(&self, session_id: Uuid) {
        loop {
            let notified = self.restart_resolution.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.has_pending_restart(session_id) {
                return;
            }
            notified.await;
        }
    }

    /// Get queue status for frontend display
    pub fn get_status(&self, session_id: Uuid) -> QueueStatus {
        match self.get_queued(session_id) {
            Some(msg) => QueueStatus::Queued { message: msg },
            None => QueueStatus::Empty,
        }
    }
}

impl Default for QueuedMessageService {
    fn default() -> Self {
        Self::new()
    }
}
