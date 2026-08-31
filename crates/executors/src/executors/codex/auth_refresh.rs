//! Serialized, up-front Codex/ChatGPT credential refresh (VAS-490).
//!
//! Codex uses rotating (single-use) OAuth refresh tokens stored in
//! `$CODEX_HOME/auth.json`. Vibe Kanban runs several `codex app-server`
//! processes at once, all sharing one account and one `auth.json`. Codex has an
//! in-process lock plus a "guarded reload" but **no cross-process file lock**,
//! so when several sessions start after the access token has expired they can
//! all try to refresh at once — the first rotates the refresh token and the
//! rest get "refresh token already used".
//!
//! This module closes that race for the dominant case: before a turn starts we
//! check whether the access token is near expiry and, if so, take a
//! cross-process advisory lock on `auth.json` and ask Codex to perform its
//! *guarded* refresh (`get_account(refresh = true)`). Serialized, exactly one
//! process performs the network refresh and writes the rotated token; every
//! other process's guarded reload sees the new token on disk and skips the
//! network call. Only the brief handshake is serialized — turns still run
//! concurrently.
//!
//! Everything here fails safe: a missing/unreadable `auth.json`, non-ChatGPT
//! (API-key) auth, a healthy token, or a lock timeout all fall back to today's
//! behavior (Codex's own lazy refresh), which is no worse than before.

use std::{
    collections::HashMap,
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::Duration as StdDuration,
};

use base64::Engine;
use chrono::{DateTime, Duration, Utc};

use super::client::AppServerClient;

/// Refresh the access token this many minutes before it expires. Mirrors
/// Codex's own `CHATGPT_ACCESS_TOKEN_REFRESH_WINDOW_MINUTES` so we only
/// pre-refresh tokens Codex would itself refresh on the next turn — never
/// introducing an otherwise-unnecessary rotation.
const REFRESH_WINDOW_MINUTES: i64 = 5;

/// Cap the cross-process lock wait. On timeout we skip the up-front refresh and
/// let Codex do its own (racy) lazy refresh — no worse than before.
const LOCK_WAIT_TIMEOUT: StdDuration = StdDuration::from_secs(10);
const LOCK_POLL_INTERVAL: StdDuration = StdDuration::from_millis(50);

/// If the ChatGPT access token in `auth.json` is near expiry, take a
/// cross-process advisory lock and drive Codex's guarded refresh once, up front.
///
/// No-op for healthy tokens, non-ChatGPT auth, or a missing/unreadable file.
pub(crate) async fn refresh_credentials_if_stale(codex_home: &Path, client: &AppServerClient) {
    let auth_path = codex_home.join("auth.json");

    // Cheap unlocked pre-check: only proceed if a ChatGPT token is present and
    // near expiry. Missing file / API-key auth / healthy token short-circuit.
    if !credential_is_stale(&auth_path, Utc::now()) {
        return;
    }

    // Serialize same-process sessions first, so at most one thread per process
    // ever contends for the cross-process file lock at a time.
    let mutex = path_mutex(&auth_path);
    let _in_process = mutex.lock().await;

    // A sibling task in this process may have refreshed while we waited.
    if !credential_is_stale(&auth_path, Utc::now()) {
        return;
    }

    // Cross-process advisory lock on the shared `auth.json` inode.
    //
    // Invariant: every actor locks the *same* inode. In the clustered worker
    // deployment each execution's scoped `auth.json` is a symlink to the shared
    // `~/.codex/auth.json` (`crates/worker/src/execution.rs::prepare_scoped_home`),
    // and `codex_home()` in the parent/worker process resolves to that same
    // shared home — so opening here (which follows the symlink) locks the one
    // shared inode for both local and clustered deployments.
    let file = match OpenOptions::new().read(true).write(true).open(&auth_path) {
        Ok(file) => file,
        Err(err) => {
            tracing::debug!("codex auth pre-refresh: cannot open {auth_path:?}: {err}");
            return;
        }
    };
    let mut lock = fd_lock::RwLock::new(file);

    let deadline = tokio::time::Instant::now() + LOCK_WAIT_TIMEOUT;
    let _file_guard = loop {
        match lock.try_write() {
            Ok(guard) => break guard,
            // Only lock contention is retryable. `try_write` maps a held lock to
            // `WouldBlock`; any other error (e.g. `ENOTSUP`/`ENOLCK` on a
            // filesystem that doesn't support advisory locks, or a transient IO
            // error) will not clear by polling, so fail fast to lazy refresh
            // rather than stalling turn startup for the whole timeout.
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if tokio::time::Instant::now() >= deadline {
                    tracing::warn!(
                        "codex auth pre-refresh: timed out waiting for auth.json lock; \
                         falling back to lazy refresh"
                    );
                    return;
                }
                tokio::time::sleep(LOCK_POLL_INTERVAL).await;
            }
            Err(err) => {
                tracing::debug!(
                    "codex auth pre-refresh: cannot lock {auth_path:?} ({err}); \
                     falling back to lazy refresh"
                );
                return;
            }
        }
    };

    // Under the lock, re-check once more: the actor that held the lock before us
    // very likely just rotated the token.
    if !credential_is_stale(&auth_path, Utc::now()) {
        return;
    }

    // Drive Codex's guarded refresh. Fail safe: a genuine, unrecoverable auth
    // failure still surfaces through the normal `get_account(false)` + turn path
    // that runs after this helper returns.
    if let Err(err) = client.get_account(true).await {
        tracing::warn!("codex auth pre-refresh failed (continuing): {err}");
    }
}

/// One async mutex per canonicalized `auth.json` path, serializing refreshers in
/// the same process.
fn path_mutex(path: &Path) -> Arc<tokio::sync::Mutex<()>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut guard = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// True iff `auth.json` holds a ChatGPT access token that expires within the
/// refresh window of `now`. Missing file / no token / parse failure → false.
fn credential_is_stale(auth_path: &Path, now: DateTime<Utc>) -> bool {
    match read_access_token(auth_path) {
        Some(token) => {
            access_token_expires_within(&token, Duration::minutes(REFRESH_WINDOW_MINUTES), now)
        }
        None => false,
    }
}

/// Read `tokens.access_token` from `auth.json`. Absent for API-key auth or when
/// not logged in.
fn read_access_token(auth_path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string(auth_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&contents).ok()?;
    json.get("tokens")?
        .get("access_token")?
        .as_str()
        .map(str::to_owned)
}

/// Whether the JWT `access_token` expires within `window` of `now`. Any
/// decode/parse failure returns `false` (fail safe: treat as not stale).
fn access_token_expires_within(access_token: &str, window: Duration, now: DateTime<Utc>) -> bool {
    match jwt_expiration(access_token) {
        Some(exp) => exp <= now + window,
        None => false,
    }
}

/// Decode the `exp` claim from a JWT's payload segment (base64url, no padding).
fn jwt_expiration(access_token: &str) -> Option<DateTime<Utc>> {
    let payload_b64 = access_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let payload: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let exp = payload.get("exp")?.as_i64()?;
    DateTime::from_timestamp(exp, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal JWT (`header.payload.signature`) whose payload carries
    /// the given `exp`. Only the payload segment is ever decoded.
    fn jwt_with_exp(exp: i64) -> String {
        let payload = serde_json::json!({ "exp": exp }).to_string();
        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload);
        format!("header.{payload_b64}.signature")
    }

    #[test]
    fn stale_when_token_expires_inside_window() {
        let now = Utc::now();
        let window = Duration::minutes(REFRESH_WINDOW_MINUTES);
        // Expires in 1 minute — well inside the 5-minute window.
        let token = jwt_with_exp((now + Duration::minutes(1)).timestamp());
        assert!(access_token_expires_within(&token, window, now));
    }

    #[test]
    fn fresh_when_token_expires_outside_window() {
        let now = Utc::now();
        let window = Duration::minutes(REFRESH_WINDOW_MINUTES);
        // Expires in 1 hour — comfortably outside the window.
        let token = jwt_with_exp((now + Duration::hours(1)).timestamp());
        assert!(!access_token_expires_within(&token, window, now));
    }

    #[test]
    fn stale_when_token_already_expired() {
        let now = Utc::now();
        let window = Duration::minutes(REFRESH_WINDOW_MINUTES);
        let token = jwt_with_exp((now - Duration::minutes(10)).timestamp());
        assert!(access_token_expires_within(&token, window, now));
    }

    #[test]
    fn unparseable_jwt_is_not_stale() {
        let now = Utc::now();
        let window = Duration::minutes(REFRESH_WINDOW_MINUTES);
        assert!(!access_token_expires_within("not-a-jwt", window, now));
        assert!(!access_token_expires_within("a.b.c", window, now));
        assert!(!access_token_expires_within("", window, now));
    }

    #[test]
    fn missing_file_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        assert!(read_access_token(&path).is_none());
        assert!(!credential_is_stale(&path, Utc::now()));
    }

    #[test]
    fn api_key_auth_without_tokens_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, r#"{"OPENAI_API_KEY":"sk-test"}"#).unwrap();
        assert!(read_access_token(&path).is_none());
        assert!(!credential_is_stale(&path, Utc::now()));
    }

    #[test]
    fn near_expiry_chatgpt_token_is_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let token = jwt_with_exp((Utc::now() + Duration::minutes(1)).timestamp());
        let contents = serde_json::json!({ "tokens": { "access_token": token } }).to_string();
        std::fs::write(&path, contents).unwrap();
        assert_eq!(read_access_token(&path).as_deref(), Some(token.as_str()));
        assert!(credential_is_stale(&path, Utc::now()));
    }

    #[test]
    fn healthy_chatgpt_token_is_not_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let token = jwt_with_exp((Utc::now() + Duration::hours(2)).timestamp());
        let contents = serde_json::json!({ "tokens": { "access_token": token } }).to_string();
        std::fs::write(&path, contents).unwrap();
        assert!(!credential_is_stale(&path, Utc::now()));
    }
}
