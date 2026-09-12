# Codex/ChatGPT credential refresh across concurrent processes

How Vibe Kanban shares one ChatGPT OAuth credential across many `codex
app-server` processes, why that produces "refresh token already used", and the
serialized up-front refresh that fixes it (VAS-490).

## The failure

Codex (ChatGPT auth) stores OAuth tokens in `$CODEX_HOME/auth.json`. OpenAI uses
**rotating, single-use refresh tokens**: a successful refresh consumes the old
refresh token and returns a new access+refresh pair. Reusing a consumed refresh
token returns `401 invalid_request` with code `refresh_token_reused`, surfaced as
*"Your access token could not be refreshed because your refresh token was already
used. Please log out and sign in again."*

The message is **Codex's, not Vibe Kanban's** — Codex owns `auth.json`; VK never
wrote it before this task.

## Why concurrency triggers it

- One turn = one `ExecutionProcess` = one `codex app-server` OS process (see
  [[agent-process-lifecycle]]). Multiple attempts / follow-ups / reviews ⇒
  multiple concurrent Codex processes.
- All share one account and one `auth.json`:
  - **local** deployment: the same `~/.codex/auth.json`;
  - **worker/cluster** deployment: each execution gets a scoped `CODEX_HOME`, but
    `prepare_scoped_home` (`crates/worker/src/execution.rs`) **symlinks**
    `auth.json` back to the shared `~/.codex/auth.json` — one inode. (Only the
    target file, e.g. `config.toml`, is snapshotted per execution; runtime assets
    including `auth.json` are symlinked so refreshes propagate.)
- Codex has an in-process async lock plus a **guarded reload** on refresh
  (`login/src/auth/manager.rs::refresh_token`: reload `auth.json`; if the on-disk
  token already changed, skip the network refresh). This closes the *same-process*
  race and the *sequential* cross-process case, but there is **no cross-process
  file lock**. Two processes that read the same not-yet-rotated token both POST →
  first wins, the rest get `refresh_token_reused`.
- Amplifier: VK's `client.get_account` was sent with `refresh_token: false`, so
  the refresh was deferred to the (long) turn, maximizing overlap.

## The fix: serialized, up-front refresh (`codex/auth_refresh.rs`)

Before the turn, in both `launch_codex_agent` (`codex.rs`) and
`launch_codex_review` (`codex/review.rs`), call
`auth_refresh::refresh_credentials_if_stale(&codex_home(), &client)`:

1. Cheap unlocked pre-check: read `auth.json`; if there's no ChatGPT
   `tokens.access_token` (API-key auth / logged out) or the access-token JWT is
   **not** within 5 min of expiry (mirrors Codex's `should_refresh_proactively`),
   do nothing — healthy tokens and non-ChatGPT auth are byte-for-byte unchanged.
2. Take a per-path **in-process** `tokio::sync::Mutex` (serializes same-process
   sessions so at most one thread per process contends for the file lock).
3. Take a **cross-process** advisory lock (`fd-lock`/`flock`) on `auth.json`
   using a **non-blocking `try_write` poll loop** with a timeout — never block the
   tokio runtime; retry only on `WouldBlock`, fail fast on other IO errors.
4. Re-check staleness under the lock; a sibling that held the lock first has
   usually just rotated the token → return without touching the network.
5. Otherwise call `client.get_account(refresh_token = true)`, which drives
   Codex's guarded refresh. Serialized, **exactly one** process performs the
   network refresh and writes the rotated token; the rest see it on the next
   guarded reload (or on the re-check in step 4) and skip it.

Everything **fails safe**: missing/unreadable file, non-ChatGPT auth, lock error,
lock timeout, or refresh error all fall back to Codex's own lazy refresh — no
worse than before. Genuine, unrecoverable auth failures still surface via the
unchanged `get_account(false)` + turn path (→ `AuthRequired`).

## Load-bearing invariants (don't break these)

- **Single shared inode.** Correctness depends on every actor locking the *same*
  `auth.json` inode. True because the worker symlinks the scoped `auth.json` to
  the shared file **and** `codex_home()` in the parent/worker process resolves to
  that same shared home. `OpenOptions::open` follows the symlink, so the `flock`
  lands on the shared inode. `flock` (not POSIX `fcntl`) is inode/OFD-based, so a
  stray `close()` elsewhere doesn't drop it.
- **Lock order = release order.** Acquire in-process mutex, then file lock;
  drop the file guard before the mutex (natural reverse-drop order). Keep it that
  way.
- **Only serialize the handshake, never the turn.** The lock is held only for the
  brief refresh, so turns still run concurrently.
- **JWT gate mirrors Codex.** Pre-refreshing outside Codex's own 5-min window
  would rotate healthy tokens unnecessarily and *increase* churn.

## Rejected alternatives

- **Full external-auth bridge** (inject tokens at `initialize`, handle
  `ChatgptAuthTokensRefresh` — currently a no-op reply of `Null` in
  `codex/client.rs`): most robust but a large, high-risk change to a sensitive
  auth surface. Deferred.
- **Reimplement OpenAI's OAuth refresh in VK:** couples us to OpenAI's
  endpoint/`client_id`/format; drift would break auth for everyone.
- **Blind retry on `refresh_token_reused`:** recovers instead of preventing;
  mid-turn retries are messy. Prevention chosen.
- **Serialize whole turns:** trivially correct, destroys concurrency.

## Residual (accepted)

Two long sessions that both cross the token lifetime at the same instant
mid-turn can still race — that refresh is inside the subprocess, outside our
lock. Rare, and mitigated by the fresh up-front token plus Codex's guarded
reload.

## Contributed by

- vk/82c6-vk-error-when-us
