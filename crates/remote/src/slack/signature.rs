//! Slack request signature verification.
//!
//! Slack signs every inbound request:
//! `X-Slack-Signature = "v0=" + hex(hmac_sha256(secret, "v0:{timestamp}:{raw_body}"))`.
//! Verification must run on the raw body bytes (before any form decoding),
//! compare in constant time, and reject stale timestamps (replay window).
//!
//! The signing secret is per-workspace-config, so the caller has to peek at
//! the payload's `team.id` to find the secret *before* verifying. That
//! ordering is safe because the peek is side-effect-free and the payload is
//! not acted on unless verification passes.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// Slack's documented replay window: reject requests whose timestamp is more
/// than five minutes from now (either direction — client clocks skew both
/// ways).
const MAX_TIMESTAMP_SKEW_SECS: i64 = 300;

/// Verify a Slack request signature.
///
/// `now_unix` is injected rather than read from the clock so tests are
/// deterministic. Returns true only when the timestamp is fresh and the
/// signature matches.
pub fn verify_slack_signature(
    signing_secret: &[u8],
    signature_header: &str,
    timestamp_header: &str,
    body: &[u8],
    now_unix: i64,
) -> bool {
    let Ok(timestamp) = timestamp_header.trim().parse::<i64>() else {
        return false;
    };
    if (now_unix - timestamp).abs() > MAX_TIMESTAMP_SKEW_SECS {
        return false;
    }

    let Some(hex_signature) = signature_header.strip_prefix("v0=") else {
        return false;
    };
    let Ok(expected_signature) = hex::decode(hex_signature) else {
        return false;
    };

    let Ok(mut mac) = HmacSha256::new_from_slice(signing_secret) else {
        return false;
    };
    mac.update(b"v0:");
    mac.update(timestamp_header.trim().as_bytes());
    mac.update(b":");
    mac.update(body);
    let computed_signature = mac.finalize().into_bytes();

    computed_signature[..].ct_eq(&expected_signature).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"8f742231b10e8888abcd99yyyzzz85a5";
    const NOW: i64 = 1_720_600_000;

    fn sign(secret: &[u8], timestamp: i64, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(format!("v0:{timestamp}:").as_bytes());
        mac.update(body);
        format!("v0={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn valid_signature() {
        let body = b"payload=%7B%22type%22%3A%22message_action%22%7D";
        let sig = sign(SECRET, NOW, body);
        assert!(verify_slack_signature(
            SECRET,
            &sig,
            &NOW.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn valid_within_skew_window() {
        let body = b"x";
        let ts = NOW - 299;
        let sig = sign(SECRET, ts, body);
        assert!(verify_slack_signature(
            SECRET,
            &sig,
            &ts.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn forged_signature_rejected() {
        let body = b"payload=x";
        let sig = "v0=0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!verify_slack_signature(
            SECRET,
            sig,
            &NOW.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn wrong_secret_rejected() {
        let body = b"payload=x";
        let sig = sign(b"other-secret", NOW, body);
        assert!(!verify_slack_signature(
            SECRET,
            &sig,
            &NOW.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn tampered_body_rejected() {
        let sig = sign(SECRET, NOW, b"payload=original");
        assert!(!verify_slack_signature(
            SECRET,
            &sig,
            &NOW.to_string(),
            b"payload=tampered",
            NOW
        ));
    }

    #[test]
    fn stale_timestamp_rejected() {
        let body = b"x";
        let ts = NOW - 301;
        let sig = sign(SECRET, ts, body);
        assert!(!verify_slack_signature(
            SECRET,
            &sig,
            &ts.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn future_timestamp_rejected() {
        let body = b"x";
        let ts = NOW + 301;
        let sig = sign(SECRET, ts, body);
        assert!(!verify_slack_signature(
            SECRET,
            &sig,
            &ts.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn missing_prefix_rejected() {
        let body = b"x";
        let sig = sign(SECRET, NOW, body);
        let without_prefix = sig.strip_prefix("v0=").unwrap();
        assert!(!verify_slack_signature(
            SECRET,
            without_prefix,
            &NOW.to_string(),
            body,
            NOW
        ));
    }

    #[test]
    fn malformed_inputs_rejected() {
        assert!(!verify_slack_signature(
            SECRET,
            "v0=not-hex",
            &NOW.to_string(),
            b"x",
            NOW
        ));
        assert!(!verify_slack_signature(
            SECRET,
            &sign(SECRET, NOW, b"x"),
            "not-a-number",
            b"x",
            NOW
        ));
        assert!(!verify_slack_signature(
            SECRET,
            &sign(SECRET, NOW, b"x"),
            "",
            b"x",
            NOW
        ));
    }
}
