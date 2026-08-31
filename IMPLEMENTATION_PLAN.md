# Implementation Plan: serialized up-front Codex credential refresh (VAS-490)

## 1. Dependency
- Add `fd-lock` (v4, already in `Cargo.lock`) to `crates/executors/Cargo.toml`.

## 2. New module `crates/executors/src/executors/codex/auth_refresh.rs`
- `pub(crate) async fn refresh_credentials_if_stale(codex_home, client)`:
  1. `auth_path = codex_home.join("auth.json")`. If missing → return (no-op).
  2. Read+parse `auth.json`; extract `tokens.access_token`. If absent (API-key
     auth) → return.
  3. `access_token_expires_within(jwt, window)` — decode JWT payload
     (base64url middle segment), read `exp`, compare to `now + window`.
     Parse failure → treat as "not stale" (fail safe) and return.
  4. If not within the window → return (healthy token, unchanged behavior).
  5. Acquire the in-process async mutex for `auth_path` (global
     `Lazy<Mutex<HashMap<PathBuf, Arc<tokio::Mutex<()>>>>>`, keyed by the
     canonicalized path).
  6. Acquire `fd-lock` exclusive on `auth.json` via `spawn_blocking`, returning
     the locked handle (held across the await).
  7. Re-read expiry under the locks; if a sibling already refreshed (now fresh)
     → drop lock, return.
  8. Call `client.get_account(refresh_token = true)` → Codex's guarded
     `refresh_token()`. Log+swallow errors (fail safe; the normal turn path
     still surfaces genuine auth failures).
  9. Drop file lock (spawn_blocking) and mutex.
- Constants: refresh window (mirror Codex's proactive window; use 5 min).
- Unit tests:
  - `expires_within` true/false around the boundary; unparseable JWT → false.
  - `auth.json` missing / no `tokens` → no-op (no panic).
  - a fresh token → helper is a no-op (assert via the pure expiry function).

## 3. `crates/executors/src/executors/codex/client.rs`
- Change `get_account` to `get_account(&self, refresh: bool)` (or add
  `get_account_refreshing`) setting `GetAccountParams { refresh_token: refresh }`.
- Update existing callers to pass `false` (behavior unchanged there).

## 4. Wire into startup — `codex.rs` and `codex/review.rs`
- In `launch_codex_agent` (codex.rs) and `launch_codex_review` (review.rs),
  before the existing `client.get_account().await?`:
  `if let Some(home) = codex_home() { auth_refresh::refresh_credentials_if_stale(&home, &client).await; }`
- Keep the existing `get_account(false)` account check afterwards.

## 5. Module registration
- Add `mod auth_refresh;` to `codex.rs`.

## 6. Verify
- `cargo test -p executors` (new unit tests + existing).
- `pnpm run check`, `pnpm run lint`, `pnpm run format`.
- `pnpm run generate-types` not needed (no shared types changed).

## Notes / invariants
- `codex_home()` in the parent/worker process resolves to the shared home whose
  `auth.json` all scoped homes symlink to — so locking it coordinates both
  deployments. No env threading needed.
- Everything fails safe to today's behavior; the worst regression is one extra
  network refresh per concurrent batch when the token is near expiry.
