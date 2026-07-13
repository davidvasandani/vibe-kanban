//! Pure prompt construction for thread summarization.
//!
//! All summarization policy lives here so it is unit-testable without a
//! network call: the system prompt, and the transcript formatter that applies
//! the FR-16 caps (bounded message count + character budget, keep the root
//! message plus the most-recent replies, mark truncation).

use crate::slack::types::SlackReplyMessage;

/// Max thread messages fetched/considered (also the `conversations.replies`
/// `limit`). Spec FR-16.
pub const MAX_THREAD_MESSAGES: usize = 100;
/// Max characters of transcript sent to Anthropic — bounds cost/latency.
/// A few thousand Haiku tokens, well within the 200K window. Spec FR-16.
pub const MAX_TRANSCRIPT_CHARS: usize = 12_000;

/// System prompt: fixed in code (no per-invocation tuning, spec Out of Scope).
pub const SYSTEM_PROMPT: &str = "You turn a Slack thread into a concise engineering issue. \
Write a short imperative title (at most about 12 words) and a description that captures the \
decision, the relevant context, and any action items. Base everything on the thread only — do \
not invent facts. If the transcript is marked truncated, summarize what is present without \
claiming it is complete.";

/// Truncation marker prepended when the middle of a long thread is dropped.
const TRUNCATION_MARKER: &str = "(earlier messages omitted for length)\n\n";

/// Format one message line. Slack user ids are opaque (`U0123`); we include
/// them only as speaker labels so the model can attribute turns.
fn format_message(msg: &SlackReplyMessage) -> Option<String> {
    let text = msg
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let speaker = msg.user.as_deref().unwrap_or("someone");
    Some(format!("{speaker}: {text}"))
}

/// Build the transcript sent as the user message, capped per FR-16.
///
/// Keeps the **root** message (index 0 — the shortcut's context) and fills
/// from the **most-recent** end within the character budget, dropping the
/// middle. Returns `None` when there is no usable text at all (caller then
/// degrades to the mechanical prefill).
pub fn build_transcript(messages: &[SlackReplyMessage]) -> Option<String> {
    let lines: Vec<String> = messages
        .iter()
        .take(MAX_THREAD_MESSAGES)
        .filter_map(format_message)
        .collect();
    if lines.is_empty() {
        return None;
    }

    // Fast path: everything fits.
    let joined = lines.join("\n");
    if joined.chars().count() <= MAX_TRANSCRIPT_CHARS {
        return Some(joined);
    }

    // Over budget: always keep the root line, then take the most-recent lines
    // that fit, and mark the gap.
    let root = &lines[0];
    let mut budget = MAX_TRANSCRIPT_CHARS.saturating_sub(
        root.chars().count() + TRUNCATION_MARKER.chars().count() + 1, // +1 for a newline
    );
    let mut tail: Vec<&String> = Vec::new();
    for line in lines[1..].iter().rev() {
        let cost = line.chars().count() + 1; // + newline
        if cost > budget {
            break;
        }
        budget -= cost;
        tail.push(line);
    }
    tail.reverse();

    let mut out = String::new();
    out.push_str(root);
    out.push('\n');
    out.push_str(TRUNCATION_MARKER);
    out.push_str(
        &tail
            .into_iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(user: &str, text: &str) -> SlackReplyMessage {
        SlackReplyMessage {
            user: Some(user.to_string()),
            text: Some(text.to_string()),
        }
    }

    #[test]
    fn empty_or_textless_thread_yields_none() {
        assert!(build_transcript(&[]).is_none());
        assert!(
            build_transcript(&[SlackReplyMessage {
                user: Some("U1".into()),
                text: None
            }])
            .is_none()
        );
    }

    #[test]
    fn short_thread_kept_verbatim_and_labeled() {
        let t = build_transcript(&[msg("U1", "root"), msg("U2", "reply")]).unwrap();
        assert_eq!(t, "U1: root\nU2: reply");
        assert!(!t.contains("omitted"));
    }

    #[test]
    fn missing_user_labeled_someone() {
        let t = build_transcript(&[SlackReplyMessage {
            user: None,
            text: Some("hi".into()),
        }])
        .unwrap();
        assert_eq!(t, "someone: hi");
    }

    #[test]
    fn over_budget_keeps_root_and_recent_with_marker() {
        // Root + many large middle messages + a small recent one.
        let big = "x".repeat(4000);
        let mut msgs = vec![msg("U0", "the original context")];
        for i in 0..10 {
            msgs.push(msg(&format!("U{i}"), &big));
        }
        msgs.push(msg("U9", "final decision: ship it"));

        let t = build_transcript(&msgs).unwrap();
        assert!(t.chars().count() <= MAX_TRANSCRIPT_CHARS);
        assert!(t.starts_with("U0: the original context"));
        assert!(t.contains("earlier messages omitted"));
        assert!(t.contains("final decision: ship it"));
    }

    #[test]
    fn message_count_cap_applies() {
        let msgs: Vec<SlackReplyMessage> = (0..MAX_THREAD_MESSAGES + 50)
            .map(|i| msg(&format!("U{i}"), &format!("m{i}")))
            .collect();
        let t = build_transcript(&msgs).unwrap();
        // The 101st message ("m100") must not appear.
        assert!(!t.contains("m100"));
        assert!(t.contains("m0"));
    }
}
