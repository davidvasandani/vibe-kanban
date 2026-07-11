//! Prefill helpers: turn a Slack message into modal initial values.
//!
//! All truncation is by `char` count on char boundaries (Slack counts
//! characters, and byte-slicing UTF-8 would panic mid-code-point).

/// Max characters for the prefilled title (spec FR-3).
const TITLE_MAX_CHARS: usize = 120;
/// Slack's hard limit for `plain_text_input.initial_value`.
const INITIAL_VALUE_MAX_CHARS: usize = 3000;

const ELLIPSIS: char = '…';

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push(ELLIPSIS);
    out
}

/// Title prefill: first non-empty line of the message, truncated to 120
/// chars. Empty/absent message text yields an empty prefill — the input is
/// required, so the user types a title (FR-8).
pub fn title_from_message(text: &str) -> String {
    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    truncate_chars(first_line, TITLE_MAX_CHARS)
}

/// Description prefill: the message text (truncated to fit Slack's
/// 3000-char initial_value cap, leaving room for the suffix) followed by a
/// permalink back to the originating message.
pub fn description_from_message(text: &str, permalink: Option<&str>) -> String {
    let suffix = permalink
        .filter(|p| !p.is_empty())
        .map(|p| format!("\n\nFrom Slack: {p}"))
        .unwrap_or_default();
    let budget = INITIAL_VALUE_MAX_CHARS.saturating_sub(suffix.chars().count());
    let body = truncate_chars(text.trim(), budget);
    format!("{body}{suffix}")
}

/// Construct the message permalink from payload fields, avoiding a
/// `chat.getPermalink` round trip inside the 3-second ack window. Format:
/// `https://{team_domain}.slack.com/archives/{channel_id}/p{ts without dot}`.
pub fn build_permalink(
    team_domain: Option<&str>,
    channel_id: &str,
    message_ts: Option<&str>,
) -> Option<String> {
    let domain = team_domain.filter(|d| !d.is_empty())?;
    let ts = message_ts.filter(|t| !t.is_empty())?;
    let ts_compact: String = ts.chars().filter(|c| *c != '.').collect();
    if channel_id.is_empty() || ts_compact.is_empty() {
        return None;
    }
    Some(format!(
        "https://{domain}.slack.com/archives/{channel_id}/p{ts_compact}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_uses_first_non_empty_line() {
        assert_eq!(
            title_from_message("\n\n  hello world  \nsecond"),
            "hello world"
        );
    }

    #[test]
    fn title_truncates_to_120_chars_with_ellipsis() {
        let long = "x".repeat(200);
        let title = title_from_message(&long);
        assert_eq!(title.chars().count(), 120);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn title_of_empty_message_is_empty() {
        assert_eq!(title_from_message(""), "");
        assert_eq!(title_from_message("\n \n"), "");
    }

    #[test]
    fn title_truncation_is_char_safe() {
        // 200 multi-byte chars: byte-slicing would panic or split a char.
        let long = "é".repeat(200);
        let title = title_from_message(&long);
        assert_eq!(title.chars().count(), 120);
    }

    #[test]
    fn description_appends_permalink() {
        let desc = description_from_message(
            "fix the login bug",
            Some("https://acme.slack.com/archives/C1/p1"),
        );
        assert_eq!(
            desc,
            "fix the login bug\n\nFrom Slack: https://acme.slack.com/archives/C1/p1"
        );
    }

    #[test]
    fn description_without_permalink_is_just_text() {
        assert_eq!(description_from_message("hello", None), "hello");
        assert_eq!(description_from_message("hello", Some("")), "hello");
    }

    #[test]
    fn description_fits_slack_initial_value_cap() {
        let long = "y".repeat(5000);
        let permalink = "https://acme.slack.com/archives/C0123456/p1720600000123456";
        let desc = description_from_message(&long, Some(permalink));
        assert!(desc.chars().count() <= 3000);
        assert!(desc.contains("From Slack: "));
        assert!(desc.ends_with(permalink));
    }

    #[test]
    fn empty_message_still_gets_permalink() {
        let desc = description_from_message("", Some("https://a.slack.com/x"));
        assert_eq!(desc, "\n\nFrom Slack: https://a.slack.com/x");
    }

    #[test]
    fn permalink_from_payload_fields() {
        assert_eq!(
            build_permalink(Some("acme"), "C0123456", Some("1720600000.123456")).as_deref(),
            Some("https://acme.slack.com/archives/C0123456/p1720600000123456")
        );
    }

    #[test]
    fn permalink_requires_domain_and_ts() {
        assert!(build_permalink(None, "C1", Some("1.2")).is_none());
        assert!(build_permalink(Some(""), "C1", Some("1.2")).is_none());
        assert!(build_permalink(Some("acme"), "C1", None).is_none());
        assert!(build_permalink(Some("acme"), "", Some("1.2")).is_none());
    }
}
