use std::{collections::VecDeque, time::SystemTime};

use cluster_protocol::{EventBatch, ExecutionEvent, ExecutionEventPayload, TerminalEvidence};
use thiserror::Error;
use uuid::Uuid;

/// A bounded, per-execution event journal.
///
/// Sequence numbers belong to the journal rather than its callers. Terminal
/// evidence is retained separately, so trimming old events can never erase the
/// worker's evidence that an execution finished.
#[derive(Debug)]
pub struct EventJournal {
    execution_id: Uuid,
    capacity: usize,
    next_sequence: u64,
    events: VecDeque<ExecutionEvent>,
    terminal: Option<TerminalEvidence>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JournalError {
    #[error("journal capacity must be greater than zero")]
    ZeroCapacity,
    #[error("event sequence is exhausted")]
    SequenceExhausted,
    #[error("execution is already terminal")]
    AlreadyTerminal,
}

impl EventJournal {
    pub fn new(execution_id: Uuid, capacity: usize) -> Result<Self, JournalError> {
        if capacity == 0 {
            return Err(JournalError::ZeroCapacity);
        }

        Ok(Self {
            execution_id,
            capacity,
            next_sequence: 1,
            events: VecDeque::with_capacity(capacity),
            terminal: None,
        })
    }

    pub fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    pub fn last_sequence(&self) -> u64 {
        self.next_sequence - 1
    }

    pub fn terminal_evidence(&self) -> Option<&TerminalEvidence> {
        self.terminal.as_ref()
    }

    /// Appends an event and returns its journal-assigned sequence number.
    ///
    /// Terminal evidence and its event are committed by the same mutation. If
    /// validation fails, neither the sequence nor retained events are changed.
    pub fn append(
        &mut self,
        worker_timestamp: SystemTime,
        payload: ExecutionEventPayload,
    ) -> Result<u64, JournalError> {
        if self.terminal.is_some() {
            return Err(JournalError::AlreadyTerminal);
        }
        let terminal = terminal_evidence(&payload);

        let sequence = self.next_sequence;
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(JournalError::SequenceExhausted)?;
        let event = ExecutionEvent {
            execution_id: self.execution_id,
            sequence,
            worker_timestamp: worker_timestamp.into(),
            payload,
        };

        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.next_sequence = next_sequence;
        if let Some(terminal) = terminal {
            self.terminal = Some(terminal);
        }

        Ok(sequence)
    }

    /// Returns retained events strictly after `after` and marks a gap when the
    /// requested next sequence has already been trimmed.
    pub fn replay_after(&self, after: u64) -> EventBatch {
        let earliest_available = self
            .events
            .front()
            .map_or(self.next_sequence, |event| event.sequence);
        let latest_available = self.last_sequence();
        let replay_gap = after.saturating_add(1) < earliest_available;
        let events = self
            .events
            .iter()
            .filter(|event| event.sequence > after)
            .cloned()
            .collect();

        EventBatch {
            execution_id: self.execution_id,
            requested_after: after,
            earliest_available,
            latest_available,
            replay_gap,
            events,
        }
    }
}

fn terminal_evidence(payload: &ExecutionEventPayload) -> Option<TerminalEvidence> {
    match payload {
        ExecutionEventPayload::Completed(evidence)
        | ExecutionEventPayload::Failed(evidence)
        | ExecutionEventPayload::Killed(evidence) => Some(evidence.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use cluster_protocol::TerminalState;

    use super::*;

    fn timestamp(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn output(value: &str) -> ExecutionEventPayload {
        ExecutionEventPayload::Stdout {
            data_base64: value.to_owned(),
        }
    }

    fn terminal(state: TerminalState, seconds: u64) -> TerminalEvidence {
        TerminalEvidence {
            state,
            exit_code: Some(0),
            signal: None,
            observed_at: timestamp(seconds).into(),
        }
    }

    #[test]
    fn assigns_monotonic_sequences_and_replays_strictly_after_cursor() {
        let id = Uuid::from_u128(1);
        let mut journal = EventJournal::new(id, 4).unwrap();
        assert_eq!(journal.append(timestamp(10), output("one")), Ok(1));
        assert_eq!(journal.append(timestamp(11), output("two")), Ok(2));
        assert_eq!(journal.append(timestamp(12), output("three")), Ok(3));

        let replay = journal.replay_after(1);
        assert_eq!(replay.execution_id, id);
        assert_eq!(replay.requested_after, 1);
        assert_eq!(replay.earliest_available, 1);
        assert_eq!(replay.latest_available, 3);
        assert!(!replay.replay_gap);
        assert_eq!(
            replay
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn bounded_history_reports_an_explicit_replay_gap() {
        let mut journal = EventJournal::new(Uuid::from_u128(2), 2).unwrap();
        journal.append(timestamp(10), output("one")).unwrap();
        journal.append(timestamp(11), output("two")).unwrap();
        journal.append(timestamp(12), output("three")).unwrap();

        let replay = journal.replay_after(0);
        assert_eq!(replay.earliest_available, 2);
        assert_eq!(replay.latest_available, 3);
        assert!(replay.replay_gap);
        assert_eq!(
            replay
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );

        let contiguous = journal.replay_after(1);
        assert!(!contiguous.replay_gap);
    }

    #[test]
    fn terminal_event_and_evidence_are_committed_together_and_evidence_is_retained() {
        let mut journal = EventJournal::new(Uuid::from_u128(3), 1).unwrap();
        journal.append(timestamp(19), output("trim me")).unwrap();
        let evidence = terminal(TerminalState::Completed, 20);
        assert_eq!(
            journal.append(
                timestamp(20),
                ExecutionEventPayload::Completed(evidence.clone())
            ),
            Ok(2)
        );

        assert_eq!(journal.terminal_evidence(), Some(&evidence));
        assert_eq!(journal.replay_after(0).events[0].sequence, 2);
        assert_eq!(
            journal.append(timestamp(21), output("too late")),
            Err(JournalError::AlreadyTerminal)
        );
        assert_eq!(journal.last_sequence(), 2);
    }

    #[test]
    fn conflicting_terminal_append_does_not_mutate_journal() {
        let mut journal = EventJournal::new(Uuid::from_u128(4), 2).unwrap();
        let completed = terminal(TerminalState::Completed, 30);
        journal
            .append(
                timestamp(30),
                ExecutionEventPayload::Completed(completed.clone()),
            )
            .unwrap();

        assert_eq!(
            journal.append(
                timestamp(31),
                ExecutionEventPayload::Failed(terminal(TerminalState::Failed, 31)),
            ),
            Err(JournalError::AlreadyTerminal)
        );
        assert_eq!(journal.last_sequence(), 1);
        assert_eq!(journal.terminal_evidence(), Some(&completed));
        assert_eq!(journal.replay_after(0).events.len(), 1);
    }

    #[test]
    fn rejects_zero_capacity() {
        assert_eq!(
            EventJournal::new(Uuid::from_u128(5), 0).unwrap_err(),
            JournalError::ZeroCapacity
        );
    }
}
