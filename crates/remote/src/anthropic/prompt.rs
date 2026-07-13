//! Pure prompt construction for thread summarization.
//!
//! All summarization policy lives here so it is unit-testable without a
//! network call: the system prompt, and the transcript formatter that applies
//! the FR-16 caps (bounded message count + character budget, keep the root
//! message plus the most-recent replies, mark truncation).

use crate::slack::types::SlackReplyMessage;

/// Max thread messages kept in the transcript (root + most-recent). Also the
/// `conversations.replies` per-page `limit`. Spec FR-16.
pub const MAX_THREAD_MESSAGES: usize = 100;
/// Max `conversations.replies` pages walked to reach the newest replies
/// (the API is oldest-first). Bounds outbound calls for pathological threads;
/// beyond this we summarize the oldest `MAX_THREAD_PAGES * MAX_THREAD_MESSAGES`
/// messages (marked truncated). Runs post-ack, so extra pages don't risk the
/// Slack deadline.
pub const MAX_THREAD_PAGES: usize = 8;
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

/// Char-truncate on a char boundary (no ellipsis — this is model input, not
/// user-facing text).
fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

/// Build the transcript sent as the user message, capped per FR-16.
///
/// `messages` are oldest-first (root at index 0), as the client accumulates
/// them across `conversations.replies` pages. Keeps the **root** message (the
/// shortcut's context) plus the **most-recent** replies, bounded by both the
/// message-count cap and the character budget; the dropped middle is marked so
/// the model doesn't treat the transcript as complete. Returns `None` when
/// there is no usable text (caller then degrades to the mechanical prefill).
pub fn build_transcript(messages: &[SlackReplyMessage]) -> Option<String> {
    let all: Vec<String> = messages.iter().filter_map(format_message).collect();
    if all.is_empty() {
        return None;
    }

    // Message-count cap (FR-16): keep the root plus the most-recent replies,
    // not the oldest `MAX_THREAD_MESSAGES` (the API is oldest-first).
    let lines: Vec<&String> = if all.len() <= MAX_THREAD_MESSAGES {
        all.iter().collect()
    } else {
        let mut v = Vec::with_capacity(MAX_THREAD_MESSAGES);
        v.push(&all[0]);
        v.extend(all[all.len() - (MAX_THREAD_MESSAGES - 1)..].iter());
        v
    };

    // Char budget (FR-16): if it all fits, keep it verbatim.
    let joined = lines
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    if joined.chars().count() <= MAX_TRANSCRIPT_CHARS {
        return Some(joined);
    }

    // Over budget: keep the root (truncated if it alone exceeds the budget —
    // otherwise `saturating_sub` would leave zero room and still emit the whole
    // root, blowing the cap) plus the most-recent lines that fit, marking the
    // dropped middle.
    let marker_len = TRUNCATION_MARKER.chars().count();
    let root_cap = MAX_TRANSCRIPT_CHARS.saturating_sub(marker_len + 1); // +1 newline
    let root_line = truncate_chars(lines[0], root_cap);
    let mut budget =
        MAX_TRANSCRIPT_CHARS.saturating_sub(root_line.chars().count() + marker_len + 1);
    let mut tail: Vec<&str> = Vec::new();
    for line in lines[1..].iter().rev() {
        let cost = line.chars().count() + 1; // + newline
        if cost > budget {
            break;
        }
        budget -= cost;
        tail.push(line.as_str());
    }
    tail.reverse();

    let mut out = String::new();
    out.push_str(&root_line);
    out.push('\n');
    out.push_str(TRUNCATION_MARKER);
    out.push_str(&tail.join("\n"));
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(user: &str, text: &str) -> SlackReplyMessage {
        SlackReplyMessage {
            user: Some(user.to_string()),
            text: Some(text.to_string()),
            ts: None,
        }
    }

    #[test]
    fn empty_or_textless_thread_yields_none() {
        assert!(build_transcript(&[]).is_none());
        assert!(
            build_transcript(&[SlackReplyMessage {
                user: Some("U1".into()),
                text: None,
                ts: None,
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
            ts: None,
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
    fn message_count_cap_keeps_root_and_most_recent() {
        let msgs: Vec<SlackReplyMessage> = (0..MAX_THREAD_MESSAGES + 50)
            .map(|i| msg(&format!("U{i}"), &format!("m{i}")))
            .collect();
        let t = build_transcript(&msgs).unwrap();
        // Root (oldest) and the newest are kept; an early-middle message is
        // dropped by the count cap.
        assert!(t.contains("U0: m0"));
        assert!(t.contains("U149: m149"));
        assert!(!t.contains("U10: m10"));
    }

    #[test]
    fn oversized_root_is_capped_to_budget() {
        // A root message larger than the whole budget must still not blow the
        // cap (Finding 3: saturating_sub → 0 previously emitted the full root).
        let root = "z".repeat(MAX_TRANSCRIPT_CHARS * 2);
        let t = build_transcript(&[msg("U0", &root), msg("U1", "reply")]).unwrap();
        assert!(t.chars().count() <= MAX_TRANSCRIPT_CHARS);
        assert!(t.contains("earlier messages omitted"));
    }
}
