# Say which 1Password lookup failed, and why

Task: `vk/76ab-debug-1password`. Vibe Kanban service only; no homelab change.

## Problem

Follow-ups in the SWE-278 workspace ("Automox Apps for Retail Devices", project
"Platform Ops", which belongs to the **Sweetgreen** organization) failed three
times on 2026-10-15 (03:55 and 05:09 UTC) with:

```
Failed to start execution: 1Password lookup failed; check the reference, service-account token, and vault permissions
```

The message names neither the variable nor the cause, so the operator cannot
tell a typo in a reference from a revoked token from a provider outage.

## Investigation (2026-10-15)

- The message is `EnvironmentSecretError::ReadFailed`, produced only when
  `op read` exits non-zero (or cannot be spawned) while
  `LocalContainerService::resolve_org_env_vars` resolves the organization's
  Env Vars. `op`'s stderr is sent to `/dev/null`.
- The Sweetgreen org has one reference: `GH_TOKEN =
  op://Homelab/alderbridge nix PAT/credential`, plus its own literal
  `OP_SERVICE_ACCOUNT_TOKEN` (which takes precedence over the host token).
- Run now with that org token: `op whoami` → the `vasandani.1password.com`
  service account; `op vault list` → `Homelab`; `op read` of the reference →
  exit 0. The reference, token and vault permission are all valid.
- The next turn (10:14 UTC) started without error. Nothing was reconfigured.
- `op service-account ratelimit` shows an **account-wide quota of 1,000
  read/write requests per 24 hours**, shared by every consumer of this service
  account (host Nix-auth refresh every 15 minutes, op-read fallback off
  Connect, GitHub owner-token resolution, and one `op read` per reference on
  every execution start). The current window opened at about 07:20 UTC, after
  the failures.

**Most likely cause:** the service account's quota was used up late in the
previous window, so every `op read` failed until the window reset. This
cannot be proven after the fact, because the CLI's stderr was thrown away.
That is the defect this task fixes.

## Goal

When a 1Password lookup fails, the execution-start error says **which**
variable failed and **what kind** of failure it was, without revealing the
reference, the token, the value, or the provider's raw output.

## Requirements

1. `op read`'s stderr is captured (bounded to 8 KiB) and classified into a
   fixed cause. The raw text is never logged, returned or stored. Quoted
   segments, where `op` echoes vault, item and field names, are removed
   first, so a name such as `token429` cannot decide the cause. Apostrophes
   inside a word do not count as quotes. The checks run in this order:
   - `NotFound`: `op`'s own phrasing, "isn't an item", "isn't a vault",
     "isn't a field" or "does not have a field". This phrasing only follows
     an authorized request, so it is checked first.
   - `RateLimited`: "rate limit", "rate-limit", "ratelimit", "too many
     requests" or "(429)".
   - `Unauthorized`: "unauthorized", "(401)", "invalid token" or
     "decodesacredentials".
   - `NotFound`: "forbidden", "(403)", "(404)" or "not found".
   - `Unknown`: anything else, or a spawn or IO failure. It keeps today's
     advice.
2. Organization Env Var failures name the variable:
   `1Password lookup for GH_TOKEN failed: <cause advice>`.
3. GitHub owner-token paths already name the owner, so they show no internal
   key: `GitHub token for acme: 1Password lookup failed: <cause advice>`.
4. A `warn` log records variable name and cause (no stderr, no reference).
5. Existing guarantees hold: no partial maps, no token in argv, ambient `OP_*`
   cleared, total 30 s timeout, child killed on cancellation, stdout bounded.

## Non-goals

- Caching resolved org Env Vars to save quota (a possible follow-up; it
  lengthens how long secrets live in memory and needs its own decision).
- Splitting the homelab service account or changing quotas (homelab repo).

## Acceptance

- Unit tests: each cause is classified from representative `op` stderr; a
  fake `op` that prints a secret-looking diagnostic yields an error whose
  Display/Debug contains neither the diagnostic nor the value; org path
  includes the variable name; GitHub paths do not include `VALUE`.
- `cargo test -p services`, `pnpm run format`, clippy clean.

## Follow-up: use the local 1Password Connect proxy (option A)

The VK server never used Connect for org Env Vars. The `vibe-kanban-dev`
unit unsets `OP_CONNECT_TOKEN` before exec, and the resolver strips every
`OP_*`. So every launch spent the cloud quota. The user chose **org
opt-in**:

- An org that sets `OP_CONNECT_HOST` and a literal `OP_CONNECT_TOKEN` gets
  Connect-first resolution (10 s per attempt), with `OP_SERVICE_ACCOUNT_TOKEN`
  (org, then ambient) as the fallback. Setting only one of the pair, or one
  as a reference, is `IncompleteConnect`.
- Ambient Connect settings are never used. Otherwise any org could read
  whatever host Connect can see.
- Each `op read` child gets exactly one credential. A Connect timeout makes
  a process-wide breaker skip that host for 60 s (a launch resolves Env Vars
  and each owner token in separate calls), and an `InvalidValue` from Connect is
  not refetched.
- Errors name the route: `… failed through 1Password Connect
  (OP_CONNECT_HOST): …` or `… through the service account
  (OP_SERVICE_ACCOUNT_TOKEN): …`.
- GitHub owner-token launch resolution forwards the org's Connect pair too.

Operator action: add `OP_CONNECT_HOST=http://172.16.100.231:8081` and the
Connect client token to each org that uses references (Sweetgreen first).
