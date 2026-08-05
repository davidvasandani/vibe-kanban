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
//!
//! Redaction runs over the **argument vector**, never over a joined string:
//! `/proc/[pid]/cmdline` is NUL-separated, an argument may contain spaces, and
//! joining first would let `--password "correct horse battery"` past the masker
//! one word at a time. [`redact_argv`] is the real entry point;
//! [`redact_command`] is a convenience for input that has already been
//! flattened and can only recover the boundaries by whitespace.
//!
//! What this heuristic does **not** catch, stated plainly so nobody mistakes it
//! for a guarantee:
//!
//! - A secret passed as a bare positional argument that does not look like
//!   credential material — anything under 20 characters, or all letters
//!   (`--` free: `mytool hunter2` is masked, `mytool trombone` is not).
//! - A flag name outside [`SECRET_KEY_FRAGMENTS`] whose value is nevertheless a
//!   secret (`--cookie`, `--pin`, a vendor's own spelling).
//! - `-p`/`-u` values that do not look like credentials; their innocent uses
//!   (`docker -p 8080:80`, `mkdir -p dir`, `ps -u root`) are far too common to
//!   mask unconditionally. See [`redact_shaped_value`].
//!
//! Those gaps are why environment variables — where secrets on this fleet
//! actually live — are not read at all rather than redacted.

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

/// Short flags whose value is *sometimes* a credential (`curl -u
/// admin:hunter2`, `mysql -phunter2`) but whose innocent uses — `docker run -p
/// 8080:80`, `mkdir -p dir`, `ps -p 1234`, `ps -u root` — are far too common
/// to mask unconditionally. Their value is masked only when it also looks like
/// credential material; see [`redact_shaped_value`].
const CREDENTIAL_SHORT_FLAGS: &[&str] = &["-p", "-u"];

/// Shortest token the shape heuristic will call a credential. Below this,
/// false positives swamp the real ones — every subcommand and short path in a
/// process table would be masked.
const MIN_CREDENTIAL_CHARS: usize = 20;

/// Redact an argument vector and render it for display.
///
/// **Each element is classified whole**, before anything is joined: an argument
/// containing spaces is one secret, not several tokens of which only the first
/// gets masked. The join is presentation only, and happens after every decision
/// has been made.
pub fn redact_argv<S: AsRef<str>>(argv: &[S]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(argv.len());
    let mut pending: Option<Pending> = None;

    for argument in argv {
        let argument = argument.as_ref();

        if let Some(pending) = pending.take() {
            out.push(match pending {
                Pending::Whole => REDACTED.to_string(),
                Pending::Shaped => redact_shaped_value(argument),
            });
            continue;
        }

        match classify(argument) {
            Token::KeyValue { key } => out.push(format!("{key}={REDACTED}")),
            Token::BareSecretKey => {
                // `--token VALUE`: the next argument is the secret, not this
                // one.
                pending = Some(Pending::Whole);
                out.push(argument.to_string());
            }
            Token::ShapedValueFlag => {
                pending = Some(Pending::Shaped);
                out.push(argument.to_string());
            }
            Token::AttachedValue { flag, value } => {
                out.push(format!("{flag}{}", redact_shaped_value(value)));
            }
            Token::UrlWithUserinfo => out.push(redact_url_userinfo(argument)),
            Token::Secret => out.push(REDACTED.to_string()),
            Token::Plain => out.push(argument.to_string()),
        }
    }

    truncate(out.join(" ").trim())
}

/// Redact a command line that has already been flattened into one string.
///
/// A convenience for callers holding a rendered command; it can only recover
/// argument boundaries by splitting on whitespace, so a multi-word secret is
/// masked word by word rather than as one argument. Collectors reading real
/// argv must call [`redact_argv`] instead.
pub fn redact_command(command: &str) -> String {
    redact_argv(&command.split_whitespace().collect::<Vec<_>>())
}

/// What the *next* argument is, once a flag has claimed it.
enum Pending {
    /// Mask it outright: the flag names a secret.
    Whole,
    /// Mask it only if it looks like credential material.
    Shaped,
}

enum Token<'a> {
    /// `--key=value` where the key names a secret.
    KeyValue {
        key: &'a str,
    },
    /// `--key` where the key names a secret and the value is the next argument.
    BareSecretKey,
    /// `-p`/`-u`: the next argument is a value worth inspecting.
    ShapedValueFlag,
    /// `-phunter2`: the same flags in their attached form.
    AttachedValue {
        flag: &'a str,
        value: &'a str,
    },
    /// `scheme://user:password@host`.
    UrlWithUserinfo,
    /// The argument itself is credential material.
    Secret,
    Plain,
}

fn classify(token: &str) -> Token<'_> {
    if let Some((key, _value)) = token.split_once('=')
        && key_is_secret(key)
    {
        return Token::KeyValue { key };
    }

    // A bare `--token` style flag only claims the *next* argument when it is a
    // flag itself; a positional argument that merely contains the word "token"
    // (a file path, say) must not swallow its neighbour.
    if token.starts_with('-') && key_is_secret(token) {
        return Token::BareSecretKey;
    }

    if CREDENTIAL_SHORT_FLAGS.contains(&token) {
        return Token::ShapedValueFlag;
    }

    if let Some((flag, value)) = split_attached_short_flag(token) {
        return Token::AttachedValue { flag, value };
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

/// `-phunter2` → `("-p", "hunter2")`.
///
/// Long flags are excluded outright: `--password=…` is already a
/// [`Token::KeyValue`], and `--pretty` is not a `-p` with a value.
fn split_attached_short_flag(token: &str) -> Option<(&str, &str)> {
    if token.starts_with("--") {
        return None;
    }
    let flag = CREDENTIAL_SHORT_FLAGS
        .iter()
        .find(|flag| token.starts_with(**flag))?;
    let value = &token[flag.len()..];
    (!value.is_empty()).then_some((*flag, value))
}

/// Mask a `-p`/`-u` value, but only where masking is not obviously wrong.
///
/// - `admin:hunter2` → `admin:«redacted»`, the `user:password` form `-u` takes.
///   A numeric right-hand side is a port pair or an id (`docker -p 8080:80`,
///   `-u 1000:1000`), not a password.
/// - `hunter2trombone` → `«redacted»`: letters *and* digits, which is what a
///   password looks like and what `build`, `1234`, and `/srv/data` do not.
///
/// Anything else is returned unchanged. `find -print` must survive this
/// function, and so must `mkdir -p build`.
fn redact_shaped_value(value: &str) -> String {
    if let Some((user, password)) = value.split_once(':') {
        if password.is_empty()
            || password
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | ':'))
        {
            return value.to_string();
        }
        return format!("{user}:{REDACTED}");
    }

    if !is_path_like(value) && mixes_letters_and_digits(value) {
        return REDACTED.to_string();
    }

    value.to_string()
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
///
/// The **last** `@` in the authority ends the userinfo, per RFC 3986: `@` is
/// legal unescaped in a password, so `postgres://user:p@ssw0rd@db/x` splits at
/// the second one. Taking the first would emit `ssw0rd` verbatim as part of the
/// "host".
fn url_userinfo_span(token: &str) -> Option<(usize, usize)> {
    let scheme_end = token.find("://")? + 3;
    let rest = &token[scheme_end..];
    // Authority ends at the first `/`; an `@` after that is part of a path.
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let at = rest[..authority_end].rfind('@')?;
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
///
/// The alphabet is **Unicode-aware** by design. An ASCII-only test would mean
/// that adding one non-ASCII character to a token exempted it from redaction —
/// making `Pässwort1234567890abc` safer to leak than its ASCII twin, which
/// inverts the bias this module exists to enforce.
fn looks_like_credential(token: &str) -> bool {
    if is_path_like(token) {
        return false;
    }
    if token.chars().count() < MIN_CREDENTIAL_CHARS {
        return false;
    }
    if !token.chars().all(is_token_char) {
        return false;
    }
    mixes_letters_and_digits(token)
}

/// The alphabet an opaque token is drawn from. Anything else — whitespace,
/// punctuation, a path separator in the middle of a word — means the argument
/// is prose or structure, not one base64/hex-ish blob.
fn is_token_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
}

/// Letters *and* digits, in any script. The cheapest signal that separates
/// `hunter2trombone` from `workspace` and from `8080`.
fn mixes_letters_and_digits(token: &str) -> bool {
    token.chars().any(char::is_alphabetic) && token.chars().any(char::is_numeric)
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

    /// An argument vector is NUL-separated, so an argument may contain spaces.
    /// Joining before redacting masked only the first word of such a secret and
    /// printed the rest verbatim — the disclosure this test exists to prevent.
    #[test]
    fn a_secret_containing_spaces_is_redacted_whole() {
        let cases: &[(&[&str], &str)] = &[
            (
                &["app", "--password", "correct horse battery staple"],
                "correct horse battery staple",
            ),
            (
                &["app", "--password=correct horse battery staple"],
                "correct horse battery staple",
            ),
            (
                &["psql", "--db_password", "hunter2 trombone stapler"],
                "hunter2 trombone stapler",
            ),
        ];

        for (argv, secret) in cases {
            let redacted = redact_argv(argv);
            for word in secret.split_whitespace() {
                assert!(
                    !redacted.contains(word),
                    "word {word:?} of secret {secret:?} survived redaction of \
                     {argv:?} as {redacted:?}"
                );
            }
            assert!(
                redacted.contains(REDACTED),
                "expected a redaction marker in {redacted:?}"
            );
        }
    }

    /// The whole argument is one unit even when it is the flag's value: an
    /// argument that merely *contains* a secret-looking word keeps its shape.
    #[test]
    fn argv_elements_are_classified_whole() {
        assert_eq!(
            redact_argv(&["app", "--token", "a b", "--port", "8080"]),
            format!("app --token {REDACTED} --port 8080")
        );
        // A rewritten-argv blob is one element and stays legible.
        assert_eq!(
            redact_argv(&["postgres: writer process   "]),
            "postgres: writer process"
        );
    }

    /// RFC 3986 ends the userinfo at the **last** `@` of the authority, and `@`
    /// is legal unescaped inside a password. Splitting at the first one printed
    /// the tail of the password as if it were the host.
    #[test]
    fn url_password_containing_at_signs_is_redacted_whole() {
        let cases: &[(&str, &str)] = &[
            ("postgres://user:p@ssw0rd@db.internal/x", "ssw0rd"),
            ("postgres://user:a@b@c@db.internal/x", "b@c"),
            ("mysql://root:@@@@@db.internal/x", "@@@@"),
        ];

        for (url, secret) in cases {
            let redacted = redact_command(url);
            assert!(
                !redacted.contains(secret),
                "secret {secret:?} survived redaction of {url:?} as {redacted:?}"
            );
            assert!(redacted.contains("db.internal"), "{redacted:?}");
        }
    }

    /// A non-ASCII character must not *exempt* a token. The heuristic used to
    /// require every character to be ASCII, so adding one accent made a token
    /// less likely to be masked than its ASCII twin.
    #[test]
    fn non_ascii_does_not_exempt_a_credential_shaped_token() {
        let redacted = redact_command("runner Pässwort1234567890abcdef --flag");
        assert!(
            !redacted.contains("Pässwort1234567890abcdef"),
            "non-ASCII token survived redaction: {redacted:?}"
        );
        assert!(redacted.ends_with("--flag"), "{redacted:?}");
    }

    /// `-u`/`-p` carry credentials often enough to be worth masking, but their
    /// innocent uses are the ones an operator opens the panel to read. Both
    /// directions are asserted together because tightening one breaks the
    /// other.
    #[test]
    fn short_credential_flags_mask_values_that_look_like_credentials() {
        let masked: &[(&[&str], &str)] = &[
            (&["curl", "-u", "admin:hunter2", "https://x/y"], "hunter2"),
            (&["mysql", "-p", "hunter2trombone"], "hunter2trombone"),
            (&["mysql", "-phunter2trombone"], "hunter2trombone"),
            (&["curl", "-uadmin:hunter2"], "hunter2"),
        ];
        for (argv, secret) in masked {
            let redacted = redact_argv(argv);
            assert!(
                !redacted.contains(secret),
                "secret {secret:?} survived redaction of {argv:?} as {redacted:?}"
            );
        }

        let untouched: &[&[&str]] = &[
            &["docker", "run", "-p", "8080:80", "image"],
            &["docker", "run", "-p", "127.0.0.1:8080:80", "image"],
            &["docker", "run", "-u", "1000:1000", "image"],
            &["mkdir", "-p", "/srv/data/incoming"],
            &["mkdir", "-p", "build"],
            &["ps", "-p", "1234"],
            &["ps", "-u", "root"],
            &["find", ".", "-print"],
        ];
        for argv in untouched {
            assert_eq!(
                redact_argv(argv),
                argv.join(" "),
                "ordinary command was altered: {argv:?}"
            );
        }
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
