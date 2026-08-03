//! Command-line redaction.
//!
//! A process table is the one place this crate can leak a credential, so
//! redaction runs *here*, inside the collector, before a
//! [`crate::types::ProcessSample`] is constructed. An unredacted command line
//! therefore never crosses the wire, never enters the coordinator's ring, and
//! never reaches a log line — which is stronger than redacting at the API edge,
//! where every intermediate copy is a place to forget.
//!
//! `/proc/[pid]/environ` is never opened anywhere in this crate. Environment
//! variables are where secrets actually live on this fleet; argv is the
//! accident. Both are treated as off-limits, but only one of them has to be
//! shown at all.
//!
//! The bias is deliberate and one-directional: an over-redacted command is a
//! cosmetic loss, an under-redacted one is a disclosure. Where a rule is
//! ambiguous, it redacts.

/// What replaces a masked value. Visibly not a value, and not something a
/// reader could mistake for the argument that was there.
pub const REDACTED: &str = "«redacted»";

/// Maximum rendered command length. A pathological argv (a `bash -c` blob, a
/// generated compiler invocation) is otherwise unbounded, and this string is
/// carried in every sample of a live stream.
pub const MAX_COMMAND_CHARS: usize = 256;

/// Argument keys whose *value* is secret. Matched case-insensitively against
/// the key, after stripping leading dashes.
///
/// These are substrings, not exact names: `--github-token`, `--db_password`,
/// and `--serviceAccountKey` all need to match, and enumerating real-world
/// spellings is a losing game.
const SECRET_KEY_FRAGMENTS: &[&str] = &[
    "apikey",
    "api-key",
    "api_key",
    "auth",
    "bearer",
    "credential",
    "passwd",
    "password",
    "privatekey",
    "private-key",
    "private_key",
    "pwd",
    "secret",
    "session",
    "token",
];

/// Vendor prefixes that identify a credential on sight, regardless of shape or
/// position. Cheaper and far more reliable than the entropy heuristic.
const SECRET_PREFIXES: &[&str] = &[
    "AKIA",        // AWS access key id
    "eyJ",         // JWT header, base64 `{"`
    "ghp_",        // GitHub personal access token
    "gho_",        // GitHub OAuth token
    "ghs_",        // GitHub app server token
    "github_pat_", // GitHub fine-grained PAT
    "glpat-",      // GitLab PAT
    "op://",       // 1Password secret reference
    "sk-",         // OpenAI / Anthropic style
    "xoxb-",       // Slack bot token
    "xoxp-",       // Slack user token
];

/// Redact a joined command line and truncate it for display.
///
/// The input is the space-joined argv produced by [`crate::parse::parse_cmdline`].
pub fn redact_command(command: &str) -> String {
    let tokens: Vec<&str> = command.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut redact_next_as_value = false;

    for token in tokens {
        if redact_next_as_value {
            redact_next_as_value = false;
            out.push(REDACTED.to_string());
            continue;
        }

        match classify(token) {
            Token::KeyValue { key } => out.push(format!("{key}={REDACTED}")),
            Token::BareSecretKey => {
                // `--token VALUE`: the next token is the secret, not this one.
                redact_next_as_value = true;
                out.push(token.to_string());
            }
            Token::UrlWithUserinfo => out.push(redact_url_userinfo(token)),
            Token::Secret => out.push(REDACTED.to_string()),
            Token::Plain => out.push(token.to_string()),
        }
    }

    truncate(&out.join(" "))
}

enum Token<'a> {
    /// `--key=value` where the key names a secret.
    KeyValue {
        key: &'a str,
    },
    /// `--key` where the key names a secret and the value is the next token.
    BareSecretKey,
    /// `scheme://user:password@host`.
    UrlWithUserinfo,
    /// The token itself is credential material.
    Secret,
    Plain,
}

fn classify(token: &str) -> Token<'_> {
    if let Some((key, _value)) = token.split_once('=')
        && key_is_secret(key)
    {
        return Token::KeyValue { key };
    }

    // A bare `--token`/`-p` style flag only claims the *next* token when it is
    // a flag itself; a positional argument that merely contains the word
    // "token" (a file path, say) must not swallow its neighbour.
    if token.starts_with('-') && key_is_secret(token) {
        return Token::BareSecretKey;
    }

    if has_secret_prefix(token) {
        return Token::Secret;
    }

    if url_userinfo_span(token).is_some() {
        return Token::UrlWithUserinfo;
    }

    if looks_like_credential(token) {
        return Token::Secret;
    }

    Token::Plain
}

fn key_is_secret(key: &str) -> bool {
    let normalized = key.trim_start_matches('-').to_ascii_lowercase();
    SECRET_KEY_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
}

fn has_secret_prefix(token: &str) -> bool {
    SECRET_PREFIXES
        .iter()
        .any(|prefix| token.starts_with(prefix))
}

/// Byte range of the `user:password` span in `scheme://user:password@host`.
///
/// Only a userinfo section carrying a password is redacted; `https://host/path`
/// and `ssh://git@host` are ordinary and stay legible.
fn url_userinfo_span(token: &str) -> Option<(usize, usize)> {
    let scheme_end = token.find("://")? + 3;
    let rest = &token[scheme_end..];
    // Authority ends at the first `/`; an `@` after that is part of a path.
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let at = rest[..authority_end].find('@')?;
    let userinfo = &rest[..at];
    userinfo
        .contains(':')
        .then_some((scheme_end, scheme_end + at))
}

fn redact_url_userinfo(token: &str) -> String {
    match url_userinfo_span(token) {
        Some((start, end)) => {
            let user = token[start..end].split_once(':').map_or("", |(u, _)| u);
            format!("{}{user}:{REDACTED}{}", &token[..start], &token[end..])
        }
        None => token.to_string(),
    }
}

/// Entropy-ish heuristic for a bare credential: a long run of token characters
/// mixing letters and digits.
///
/// **Filesystem paths are deliberately exempt.** On this fleet every command is
/// a `/nix/store/<32-char-hash>-name` path, which matches the shape rule
/// exactly. Redacting those would replace the entire process table with
/// `«redacted»` and destroy the feature to protect nothing — a store path is
/// public by construction. Paths are still redacted when they match a secret
/// key, a vendor prefix, or carry URL userinfo, so the exemption removes a
/// false positive rather than a real rule.
fn looks_like_credential(token: &str) -> bool {
    if is_path_like(token) {
        return false;
    }
    if token.len() < 20 {
        return false;
    }
    if !token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-'))
    {
        return false;
    }
    let has_digit = token.chars().any(|c| c.is_ascii_digit());
    let has_alpha = token.chars().any(|c| c.is_ascii_alphabetic());
    has_digit && has_alpha
}

fn is_path_like(token: &str) -> bool {
    token.starts_with('/') || token.starts_with("./") || token.starts_with("../")
}

/// Truncate on a character boundary, appending an ellipsis so the reader knows
/// the command continued.
fn truncate(command: &str) -> String {
    if command.chars().count() <= MAX_COMMAND_CHARS {
        return command.to_string();
    }
    let mut out: String = command.chars().take(MAX_COMMAND_CHARS).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every case pairs a realistic command with the secret substring that must
    /// not survive. Asserting on the *absence of the secret* rather than on an
    /// exact expected string is deliberate: it keeps passing when the redaction
    /// format changes, and it fails for the only reason that matters.
    #[test]
    fn planted_secrets_never_survive() {
        let cases: &[(&str, &str)] = &[
            (
                "node server.js --api-key=sk-live-abc123def456ghi789",
                "sk-live-abc123def456ghi789",
            ),
            (
                "psql --password hunter2trombone --host db.internal",
                "hunter2trombone",
            ),
            (
                "curl https://alice:s3cr3tpw@api.example.com/v1/items",
                "s3cr3tpw",
            ),
            (
                "deploy --github-token ghp_16CharsAndMoreHere0987654321",
                "ghp_16CharsAndMoreHere0987654321",
            ),
            (
                "aws --credential AKIAIOSFODNN7EXAMPLE s3 ls",
                "AKIAIOSFODNN7EXAMPLE",
            ),
            ("app --db_password=p4ssw0rd --verbose", "p4ssw0rd"),
            (
                "slack-mcp --token xoxb-123456789012-abcdefghijkl",
                "xoxb-123456789012-abcdefghijkl",
            ),
            (
                "worker --session eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
                "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            ),
            (
                "op read op://Homelab/Claude/token",
                "op://Homelab/Claude/token",
            ),
            (
                "runner AbCdEf0123456789GhIjKlMn --flag",
                "AbCdEf0123456789GhIjKlMn",
            ),
            (
                "svc --AUTH_TOKEN=MixedCase1234567890abcdef",
                "MixedCase1234567890abcdef",
            ),
        ];

        for (command, secret) in cases {
            let redacted = redact_command(command);
            assert!(
                !redacted.contains(secret),
                "secret {secret:?} survived redaction of {command:?} as {redacted:?}"
            );
            assert!(
                redacted.contains(REDACTED),
                "expected a redaction marker in {redacted:?}"
            );
        }
    }

    /// The other half of the contract: redaction that eats everything is not
    /// "safe", it is a broken feature. These are the commands an operator opens
    /// the panel to read.
    #[test]
    fn ordinary_commands_survive_intact() {
        let cases = [
            "/srv/vk-releases/current/bin/vibe-kanban",
            "/nix/store/c1cjgg6p8m8fssivzrc2p13mwwml3p3v-findutils-4.10.0/bin/find",
            "postgres: vibe_kanban_remote remote ::1(54510) idle",
            "cargo build --release --workspace",
            "/nix/store/z1cxjx705fswwjjns0sw2ysbd5jqxfgm-bun-1.3.13/bin/bun run dev",
            "ssh://git@github.com/owner/repo.git",
            "https://api.example.com/v1/items",
        ];

        for command in cases {
            let redacted = redact_command(command);
            assert_eq!(
                redacted, command,
                "ordinary command was altered: {command:?} -> {redacted:?}"
            );
        }
    }

    /// A nix store hash is 32 characters of base32 inside a path — exactly the
    /// credential shape. Guarding it explicitly because losing this exemption
    /// silently reduces the whole process table to `«redacted»` on every host in
    /// this fleet.
    #[test]
    fn nix_store_paths_are_not_mistaken_for_credentials() {
        let path = "/nix/store/mm1a1wnphf568znv8jsz1gf4476yjhzm-nodejs-slim-24.15.0/bin/node";
        assert_eq!(redact_command(path), path);
    }

    /// `--token VALUE` hides the *next* token. Getting this wrong leaks the
    /// most common secret-passing convention there is.
    #[test]
    fn bare_flag_redacts_the_following_value() {
        let redacted = redact_command("app --token supersecret --port 8080");
        assert_eq!(redacted, format!("app --token {REDACTED} --port 8080"));
    }

    /// A positional argument that merely contains the word "token" must not
    /// swallow its neighbour — otherwise reading a token *file path* hides an
    /// unrelated argument and the output silently lies about the command.
    #[test]
    fn non_flag_token_word_does_not_consume_its_neighbour() {
        let redacted = redact_command("cat tokens.txt --port 8080");
        assert_eq!(redacted, "cat tokens.txt --port 8080");
    }

    #[test]
    fn url_userinfo_keeps_the_user_and_hides_the_password() {
        let redacted = redact_command("curl https://alice:s3cr3t@example.com/x");
        assert_eq!(
            redacted,
            format!("curl https://alice:{REDACTED}@example.com/x")
        );
    }

    #[test]
    fn long_commands_are_truncated_on_a_character_boundary() {
        let command = "x".repeat(MAX_COMMAND_CHARS + 50);
        let redacted = redact_command(&command);
        assert_eq!(redacted.chars().count(), MAX_COMMAND_CHARS + 1);
        assert!(redacted.ends_with('…'));
    }

    /// Multi-byte input must not panic on truncation — `String::truncate` on a
    /// byte index would.
    #[test]
    fn truncation_handles_multibyte_characters() {
        let command = "é".repeat(MAX_COMMAND_CHARS + 10);
        let redacted = redact_command(&command);
        assert_eq!(redacted.chars().count(), MAX_COMMAND_CHARS + 1);
    }

    #[test]
    fn empty_command_stays_empty() {
        assert_eq!(redact_command(""), "");
        assert_eq!(redact_command("   "), "");
    }
}
