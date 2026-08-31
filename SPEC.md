# Spec: Fix Codex/ChatGPT "refresh token already used" error

Task: VAS-490 — Vibe Kanban error when using ChatGPT (Codex executor):

> Error: Your access token could not be refreshed because your refresh token was
> already used. Please log out and sign in again.

## Problem

The error text is produced by the **Codex CLI subprocess** (`codex app-server`,
pinned to `@openai/codex@0.144.1`), not by Vibe Kanban. Codex manages the
ChatGPT OAuth tokens in `$CODEX_HOME/auth.json`. OpenAI issues **rotating
(one-time-use) refresh tokens**: each successful refresh consumes the old
refresh token and returns a new access+refresh pair. Reusing a consumed refresh
token returns `invalid_grant` with code `refresh_token_reused`, which Codex
surfaces as the message above.

Vibe Kanban launches **multiple concurrent `codex app-server` processes** that
all share one account and one `auth.json`:

- Local deployment: every process uses the same `~/.codex/auth.json` directly.
- Worker/cluster deployment: each execution gets a scoped `CODEX_HOME`, but
  `auth.json` is **symlinked back** to the shared `~/.codex/auth.json`
  (`crates/worker/src/execution.rs`, `prepare_scoped_home`). The symlink target
  is a single shared inode.

Codex has an in-process mutex plus a "guarded reload" (on refresh it re-reads
`auth.json`; if another actor already rotated the token it skips the network
refresh — `login/src/auth/manager.rs::refresh_token`). This closes the
single-process race but there is **no cross-process file lock**. When several
Codex processes start after the access token has expired, each one's first turn
triggers a refresh; they read the same not-yet-rotated token, all POST to the
token endpoint, the first wins and the rest get `refresh_token_reused`.

Vibe Kanban's `get_account()` is currently sent with `refresh_token: false`, so
the refresh is deferred to the (long-running) turn rather than done up front —
maximizing the concurrency window.

## Root cause

Concurrent Codex processes sharing one rotating refresh token, with the refresh
performed lazily during the turn and **not serialized across processes**.

## Goal

Prevent the reuse race for the dominant real-world trigger — several tasks
started at once after the access token has expired — without serializing the
actual agent turns (which would kill concurrency) and without re-implementing
OpenAI's OAuth flow inside Vibe Kanban.

## Approach

Add a **serialized, up-front credential refresh** to the Codex startup handshake
(before `thread_start`/`turn_start`), in both the agent and review paths:

1. Locate the shared `auth.json` via the existing `codex_home()` helper (this
   resolves to the same shared file both deployments use).
2. If it is absent or not a ChatGPT token, or its access-token JWT is **not**
   near expiry, do nothing — behavior is unchanged for healthy tokens.
3. If the access token is near expiry (mirroring Codex's proactive window),
   acquire a **cross-process advisory file lock (`flock`)** on `auth.json`, then:
   - re-check expiry under the lock (a sibling process may have just refreshed;
     if now fresh, skip);
   - otherwise call `get_account(refresh_token: true)`, which invokes Codex's
     **guarded** `refresh_token()`. Under the lock exactly one process performs
     the network refresh and writes the rotated token; the others see the
     changed on-disk token via the guarded reload and skip the network call.
4. Release the lock and proceed. Turns then run concurrently with a fresh token.

An in-process async mutex (keyed by the canonical `auth.json` path) fronts the
`flock` so same-process concurrency is serialized in async-land without parking
blocking threads.

### Why this is safe

- Every codex-coupling assumption fails safe: missing/unparseable `auth.json`,
  non-ChatGPT auth, or a lock error all fall back to today's behavior (no worse
  than the status quo).
- No OAuth protocol is reimplemented; Codex still owns the refresh. We only
  serialize *when* it happens and move it earlier.
- The lock is held only for the brief auth handshake, never for a turn.

### Residual (accepted, documented)

Two long turns that both cross the token TTL at the same instant mid-turn can
still race (the mid-turn refresh is inside the subprocess, outside our lock).
This is rare because the up-front refresh makes the token fresh for the whole
session; Codex's own guarded reload continues to mitigate it.

## Scope

- `crates/executors/src/executors/codex/auth_refresh.rs` (new): expiry check,
  locking, orchestration, unit tests.
- `crates/executors/src/executors/codex/client.rs`: allow `get_account` to
  request a refresh.
- `crates/executors/src/executors/codex.rs` + `codex/review.rs`: call the helper
  before the existing `get_account()`.
- `crates/executors/Cargo.toml`: add `fd-lock` (already in the lockfile).

Out of scope: bumping `@openai/codex` (Renovate-managed), the full external-auth
bridge, and UI changes.

## Acceptance criteria

- Starting several Codex tasks at once with a near-expired ChatGPT token no
  longer produces `refresh_token_reused`; at most one network refresh occurs.
- Healthy (not-near-expiry) tokens see no new refresh and no behavior change.
- API-key / non-ChatGPT auth and logged-out states are unaffected.
- `cargo test -p executors`, `pnpm run check`, `pnpm run lint` pass.
