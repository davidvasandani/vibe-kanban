//! Automatic backoff for provider capacity errors (VK constitution LX).
//!
//! When a coding-agent turn ends with "Selected model is at capacity", the
//! work is unfinished and the session would otherwise wait for a human. This
//! module holds the pure policy: what counts as evidence, how long each
//! attempt waits, when the chain stops, and what the resumed agent is told.
//! The server's consumer (`server::capacity_backoff`) schedules a durable
//! wake-up from it, and the shared dispatcher delivers that wake-up.

use std::time::Duration;

use executors::logs::{NormalizedEntry, NormalizedEntryType};

/// Lowercase substrings that identify a provider capacity error. Add a
/// phrasing here only once its exact text and transient meaning are confirmed.
pub const CAPACITY_ERROR_PATTERNS: &[&str] = &["selected model is at capacity"];

/// Largest share of the delay added as jitter, so workspaces that failed
/// together do not all retry in the same second.
pub const MAX_JITTER_FRACTION: f64 = 0.2;

pub fn is_capacity_error(text: &str) -> bool {
    let text = text.to_lowercase();
    CAPACITY_ERROR_PATTERNS
        .iter()
        .any(|pattern| text.contains(pattern))
}

/// True when the turn's last word was a capacity error: an error entry
/// matching [`is_capacity_error`] with no agent message or tool use after it.
/// The same text in user, assistant or tool content is never evidence, and an
/// earlier capacity error the agent recovered from does not count.
pub fn turn_ended_with_capacity_error(entries: &[NormalizedEntry]) -> bool {
    for entry in entries.iter().rev() {
        match entry.entry_type {
            NormalizedEntryType::ErrorMessage { .. } if is_capacity_error(&entry.content) => {
                return true;
            }
            NormalizedEntryType::AssistantMessage | NormalizedEntryType::ToolUse { .. } => {
                return false;
            }
            _ => {}
        }
    }
    false
}

/// Wait before 1-based `attempt`: `base` doubled `attempt - 1` times, never
/// above `max`. The cap wins even when it is below `base`.
pub fn backoff_delay(attempt: u32, base: Duration, max: Duration) -> Duration {
    let doublings = attempt.saturating_sub(1);
    let factor = 1u32.checked_shl(doublings).unwrap_or(u32::MAX);
    base.max(Duration::from_secs(1))
        .checked_mul(factor)
        .unwrap_or(max)
        .min(max)
}

/// Add up to [`MAX_JITTER_FRACTION`] of `delay`; `fraction` is a random value
/// in `[0, 1]` supplied by the caller.
pub fn with_jitter(delay: Duration, fraction: f64) -> Duration {
    let fraction = if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    };
    delay + delay.mul_f64(MAX_JITTER_FRACTION * fraction)
}

/// The attempt to schedule after a capacity failure. `previous` is the attempt
/// number of the retry that started the failed turn, or `None` when the turn
/// was started by anything else. `None` means the chain is exhausted.
pub fn next_attempt(previous: Option<u32>, max_attempts: u32) -> Option<u32> {
    let next = match previous {
        Some(attempt) => attempt.checked_add(1)?,
        None => 1,
    };
    (next <= max_attempts).then_some(next)
}

/// System-authored prompt delivered when a retry fires.
pub fn retry_message(attempt: u32, max_attempts: u32) -> String {
    format!(
        "Automatic capacity retry (attempt {attempt} of {max_attempts}). The previous turn did not \
         finish: it ended because the model provider reported that the selected model was at \
         capacity. Vibe Kanban waited and is resuming this session automatically. Review the \
         current state of the working tree and continue the task from where it left off."
    )
}

#[cfg(test)]
mod tests {
    use executors::logs::{ActionType, NormalizedEntryError, ToolStatus};

    use super::*;

    const CAPACITY: &str = "Error: Selected model is at capacity. Please try a different model.";

    fn entry(entry_type: NormalizedEntryType, content: &str) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type,
            content: content.to_string(),
            metadata: None,
        }
    }

    fn error(content: &str) -> NormalizedEntry {
        entry(
            NormalizedEntryType::ErrorMessage {
                error_type: NormalizedEntryError::Other,
            },
            content,
        )
    }

    fn tool() -> NormalizedEntry {
        entry(
            NormalizedEntryType::ToolUse {
                tool_name: "bash".into(),
                action_type: ActionType::Other {
                    description: "run".into(),
                },
                status: ToolStatus::Success,
            },
            CAPACITY,
        )
    }

    #[test]
    fn matches_the_reported_text_in_any_case() {
        assert!(is_capacity_error(CAPACITY));
        assert!(is_capacity_error("SELECTED MODEL IS AT CAPACITY"));
        assert!(!is_capacity_error("API Error: 529 overloaded_error"));
        assert!(!is_capacity_error(""));
    }

    #[test]
    fn a_trailing_capacity_error_is_evidence() {
        let entries = vec![
            entry(NormalizedEntryType::UserMessage, "go"),
            entry(NormalizedEntryType::AssistantMessage, "working"),
            error(CAPACITY),
        ];
        assert!(turn_ended_with_capacity_error(&entries));
    }

    #[test]
    fn entries_that_are_not_agent_output_may_follow_the_error() {
        let entries = vec![
            error(CAPACITY),
            entry(NormalizedEntryType::SystemMessage, "tokens used"),
            error("Error: stream closed"),
        ];
        assert!(turn_ended_with_capacity_error(&entries));
    }

    #[test]
    fn agent_output_after_the_error_means_the_turn_recovered() {
        let recovered = vec![
            error(CAPACITY),
            entry(NormalizedEntryType::AssistantMessage, "done"),
        ];
        assert!(!turn_ended_with_capacity_error(&recovered));
        assert!(!turn_ended_with_capacity_error(&[error(CAPACITY), tool()]));
    }

    #[test]
    fn the_phrase_outside_an_error_entry_is_never_evidence() {
        for entry_type in [
            NormalizedEntryType::UserMessage,
            NormalizedEntryType::AssistantMessage,
            NormalizedEntryType::SystemMessage,
            NormalizedEntryType::Thinking,
        ] {
            assert!(!turn_ended_with_capacity_error(&[entry(
                entry_type, CAPACITY
            )]));
        }
        assert!(!turn_ended_with_capacity_error(&[tool()]));
        assert!(!turn_ended_with_capacity_error(&[error("Error: boom")]));
        assert!(!turn_ended_with_capacity_error(&[]));
    }

    #[test]
    fn delay_doubles_to_the_cap() {
        let base = Duration::from_secs(60);
        let max = Duration::from_secs(3600);
        let delays: Vec<u64> = (1..=10)
            .map(|attempt| backoff_delay(attempt, base, max).as_secs())
            .collect();
        assert_eq!(
            delays,
            [60, 120, 240, 480, 960, 1920, 3600, 3600, 3600, 3600]
        );
    }

    #[test]
    fn delay_handles_degenerate_inputs() {
        let max = Duration::from_secs(3600);
        // Attempt 0 is treated as the first attempt.
        assert_eq!(backoff_delay(0, Duration::from_secs(60), max).as_secs(), 60);
        // Huge attempts saturate instead of overflowing.
        assert_eq!(backoff_delay(u32::MAX, Duration::from_secs(60), max), max);
        assert_eq!(backoff_delay(40, Duration::from_secs(60), max), max);
        // A zero base still waits.
        assert_eq!(backoff_delay(1, Duration::ZERO, max).as_secs(), 1);
        // The cap wins even below the base.
        assert_eq!(
            backoff_delay(1, Duration::from_secs(60), Duration::from_secs(10)).as_secs(),
            10
        );
    }

    #[test]
    fn jitter_adds_at_most_a_fifth() {
        let delay = Duration::from_secs(100);
        assert_eq!(with_jitter(delay, 0.0), delay);
        assert_eq!(with_jitter(delay, 1.0), Duration::from_secs(120));
        assert_eq!(with_jitter(delay, 0.5), Duration::from_secs(110));
        assert_eq!(with_jitter(delay, 7.0), Duration::from_secs(120));
        assert_eq!(with_jitter(delay, -1.0), delay);
        assert_eq!(with_jitter(delay, f64::NAN), delay);
    }

    #[test]
    fn attempts_chain_and_stop_at_the_limit() {
        assert_eq!(next_attempt(None, 10), Some(1));
        assert_eq!(next_attempt(Some(1), 10), Some(2));
        assert_eq!(next_attempt(Some(9), 10), Some(10));
        assert_eq!(next_attempt(Some(10), 10), None);
        assert_eq!(next_attempt(None, 0), None);
        assert_eq!(next_attempt(Some(u32::MAX), u32::MAX), None);
    }

    #[test]
    fn message_names_the_attempt_and_the_cause() {
        let message = retry_message(3, 10);
        assert!(message.contains("attempt 3 of 10"));
        assert!(message.contains("at capacity"));
        // The message must not look like the error it reports, or a quoted
        // copy in a later log could be mistaken for one.
        assert!(!is_capacity_error(&message));
    }
}
