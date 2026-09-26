# Prior knowledge: vk/6c79-codex-auth-error

Distilled from the two knowledge bases (read-only):

- `homelab/docs/knowledge-base/cluster-shared-credential-freshness.md` (vk/2512)
- `homelab/docs/knowledge-base/vibe-kanban-worker-start-recovery.md` (vk/611f, vk/53c3, vk/2512)
- `vibe-kanban/wiki/codex-credential-refresh.md` (vk/82c6, VAS-490)

## Credential topology (homelab)

- think2 owns the Codex ChatGPT login: the only real, single-use, rotating
  refresh token. `codex-auth-sync` (every 5 min) copies `auth.json` to other
  nodes with `tokens.refresh_token = ""`. It has to be blanked rather than
  deleted, because the field is a required String in Codex's `TokenData`.
- `codex-auth-freshness` renews on think2 by driving the vendor CLI
  (`codex exec --ignore-user-config`) when fewer than 2 days of a 10-day life
  remain. It pages on any node below 1 day. **Renewal goes through the vendor
  CLI, never a hand-rolled token POST.** A rotating exchange that dies midway
  loses the credential for every node.
- `vibe-kanban-worker` has `Requires=codex-auth-sync`. A failed required unit
  blocks the worker's start job, and systemd doesn't requeue it. **New
  Codex-only units must have no dependents.** Assert that against the real
  `nixosConfigurations`, with a positive control.
- `writeShellApplication` pins PATH to `runtimeInputs` (`cmp` is in
  `diffutils`). A log line that always prints isn't a signal.
- Never emit token material. Report only `exp`, durations and field lengths, and
  suppress decoder stderr.
- Test against a scratch `CODEX_HOME`. Rewriting the JWT payload locally drives
  client decisions without the crafted token reaching a server.
- `codex doctor` permanently reports "stored credentials are incomplete" on
  workers. That's by design, so it's useless as a health check.

## Vibe Kanban executor (vibe-kanban)

- Each turn runs one `codex app-server` process. On workers, the scoped
  `CODEX_HOME/auth.json` is a symlink to the shared `~/.codex/auth.json` (one
  inode).
- `codex/auth_refresh.rs` does a serialized pre-turn refresh (flock on
  `auth.json`) when the JWT is within 5 minutes of expiry, using
  `get_account(refresh=true)`. It fails safe.
- Previously rejected:
  - Reimplementing OpenAI OAuth refresh in VK, because of drift and coupling.
  - A full external-auth bridge (`ChatgptAuthTokensRefresh`, currently answered
    with `Null`), as too large and risky.
- Errors: Codex `error` notifications are normalized as `ErrorMessage { Other }`.
  `SetupRequired` exists and is used for auth-required launch errors.

## Implications for this task

- Fix at the distribution layer. Serving think2's already-renewed token to a
  worker isn't a refresh reimplementation: no token is exchanged or rotated, so
  it respects both "no hand-rolled refresh" rules.
- The broker unit must have no dependents, and its env wiring must not be
  conditioned on it being up.
- Keep the VK change small and fail-safe. Don't add the external-auth bridge.
