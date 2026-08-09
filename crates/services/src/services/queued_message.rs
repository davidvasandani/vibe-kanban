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
    restart_resolution: Arc<Notify>,
}

impl QueuedMessageService {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(DashMap::new()),
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
        match self.queue.entry(session_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().restart_reservation = Some(reservation);
                entry.get_mut().remove_on_reservation_cancel = false;
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
                entry.insert(queued);
            }
        }
        reservation
    }

    pub fn commit_mcp_restart(&self, session_id: Uuid, reservation: Uuid) -> Option<DateTime<Utc>> {
        let result = match self.queue.entry(session_id) {
            Entry::Occupied(mut entry) if entry.get().restart_reservation == Some(reservation) => {
                entry.get_mut().restart_agent = true;
                entry.get_mut().restart_reservation = None;
                entry.get_mut().remove_on_reservation_cancel = false;
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
