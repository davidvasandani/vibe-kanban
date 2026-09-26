# SPEC: Codex auth error on cluster workers (vk/6c79-codex-auth-error)

Spans two repos: `homelab` (credential distribution, the primary fix) and
`vibe-kanban` (executor behaviour). This file is identical in both.

## Problem

Agents on think cluster workers intermittently lose Codex turns with:

```
Failed to refresh token: 400 Bad Request: Invalid 'refresh_token': empty string.
```

Seen in about 50 Codex rollouts on think1/3/4/5 since 2026-09-20, from both the
Vibe Kanban Codex executor (`vibe-codex-executor`) and agents that run the
Codex CLI directly (`codex exec` / `codex review`, e.g. the pipeline's review
stage).

### Root cause (verified)

1. `modules/codex-auth-sync.nix` copies think2's `auth.json` to every other node
   with `tokens.refresh_token` blanked (VAS-490). The refresh token is single-use
   and rotating, so only think2 can hold it. This part is correct.
2. The module assumes a worker that reaches the refresh path "fails harmlessly
   and keeps using its synced access token". **That's false for a 401.** In
   Codex 0.155.1, a 401 from the model endpoint runs `UnauthorizedRecovery`:
   first a reload of `auth.json`, then a network refresh. With a blank refresh
   token the refresh returns 400, and the turn fails with no retry.
3. Workers get 401s even while their access token is unexpired. Failures come in
   bursts (e.g. 2026-09-25 23:00–23:16), and the same token works before and
   after. Workers can also hold an older token than think2 for a while after
   think2 renews (think2 renewed at 2026-09-23 18:19, workers picked it up
   ~21:10). Both cases end in the same fatal refresh.

### Verified mechanism for the fix

Codex reads `CODEX_REFRESH_TOKEN_URL_OVERRIDE` for its refresh endpoint.
Measured on 0.155.1 with a scratch `CODEX_HOME` whose access token was
corrupted (real upstream 401):

- With the override pointing at a local endpoint that returns
  `{id_token, access_token}`, Codex called it (`grant_type=refresh_token`,
  empty refresh token) and persisted the returned access token. It kept the
  blank refresh token, retried, and the turn succeeded (exit 0).
- Without the override, the same setup failed (exit 1), reproducing the
  incident.

## Goals

- G1: On a non-canonical node, a Codex 401 recovers by adopting think2's
  current credential instead of failing the turn. This covers every Codex caller
  on the node: the VK executor, CLI calls from agents, and the credential owner's shells.
- G2: A worker never spends, rotates or revokes think2's refresh token (VAS-490
  stays intact).
- G3: When recovery is impossible (think2's credential is itself expired, or
  think2 is unreachable), the failure says so plainly instead of reporting an
  "empty string" 400.
- G4: The Vibe Kanban executor understands this deployment contract. It turns
  the refresh failure into an actionable setup error, and it doesn't drive a
  doomed pre-turn refresh when the credential can't be refreshed.

## Non-goals

- Explaining or eliminating the upstream 401 bursts (OpenAI-side).
- Renewing think2's credential on demand. Renewal stays with
  `codex-auth-freshness` on think2.
- Changing the canonical node's behaviour. think2 keeps its real refresh token
  and gets no override.

## Requirements — homelab (primary fix)

- H1 **Refresh broker.** New `codex-refresh-broker` service on every node where
  `services.codexAuthSync.enable` is true and the node is **not** the canonical
  source. It's a Python stdlib HTTP server bound to `127.0.0.1:<port>`, running
  as the Codex user with the same hardening as `codex-auth-sync`.
- H2 **POST `/oauth/token`.**
  - Reject callers whose socket isn't owned by the broker's own uid (403),
    found via `/proc/net/tcp{,6}`. The endpoint hands out a bearer credential,
    so other local users must not get it.
  - Require `grant_type == "refresh_token"` (400 otherwise).
  - Single-flight the pull: concurrent requests share one canonical pull, and
    a pull that finished within a short minimum interval is reused.
  - Pull by running the existing `codex-auth-sync` program. That reuses its
    source, validation, blanking and atomic install, so there's no second copy
    of that logic.
  - Serve from the resulting local `auth.json`:
    - `200 {"id_token","access_token"}` when the access token has more than a
      small margin of life left. Never include `refresh_token`, so Codex keeps
      the blank one.
    - `503` with a JSON `error.code`/`error.message` when the pull fails or the
      credential is expired or unreadable. The message names think2 as the
      owner of renewal. Never use 401 or a code Codex recognises: Codex
      replaces those messages with its canned "log out and sign in again" text
      (see `specs/vk/6c79-codex-auth-error/research.md` R3).
- H3 **POST `/oauth/revoke`** returns 200 and does nothing. Codex derives its
  revoke URL from the refresh override, so `codex logout` on a worker must not
  reach OpenAI and revoke the shared login.
- H4 Any other path or method returns 404/405. Logs never contain token
  material: only outcome, remaining lifetime in seconds, and caller uid.
- H5 **Wiring.** On nodes that run the broker, export
  `CODEX_REFRESH_TOKEN_URL_OVERRIDE=http://127.0.0.1:<port>/oauth/token` to:
  - the `vibe-kanban-worker` service environment, next to `CODEX_HOME`;
  - login shells of the credential's own account only (`environment.extraInit`
    guarded by `id -un`). Other users' personal Codex logins must keep
    refreshing against OpenAI.

  Never export it on the canonical node.
- H6 **Isolation.** No unit may `Requires=`/`BindsTo=`/`PartOf=` the broker. A
  Codex-only fault must not stop the worker (see
  `vibe-kanban-worker-start-recovery.md`). If the broker is down, Codex gets a
  connection error, which is no worse than today.
- H7 **Tests.**
  - Python unit tests for the broker: uid gate, grant check, response shaping
    (no `refresh_token`), expiry to 503, pull failure to 503, single-flight
    reuse, revoke no-op.
  - Nix eval assertions: broker and override present on workers, absent on the
    source node, and no unit depends on the broker.
  - Both wired into `deploy-invariants.yml`.
- H8 **Docs.** Correct the "fails harmlessly" comment in `codex-auth-sync.nix`,
  and update the knowledge-base page on shared credential freshness.

## Requirements — vibe-kanban

- V1 **Actionable error.** When Codex reports an error whose message is a
  refresh-token failure (`Failed to refresh token` / invalid or empty
  `refresh_token`), in both the v2 `error` notification and legacy
  `EventMsg::Error`:
  - render it as `NormalizedEntryError::SetupRequired`;
  - keep the original message;
  - explain that the access token was rejected and this node can't refresh it
    locally;
  - point to the fix: the node's refresh endpoint
    (`CODEX_REFRESH_TOKEN_URL_OVERRIDE`) or `codex login` on the credential's
    owner.

  Other errors are unchanged.
- V2 **No doomed pre-turn refresh.** `auth_refresh::refresh_credentials_if_stale`
  skips `get_account(refresh=true)`, with one clear warning, when the on-disk
  ChatGPT credential has an empty refresh token and
  `CODEX_REFRESH_TOKEN_URL_OVERRIDE` is unset. With the override set, keep the
  current behaviour: the refresh goes to the broker and adopts think2's token.
- V3 Unit tests for V1's classification and V2's decision function.

## Acceptance

- A1 On a worker, a Codex run whose access token is rejected with 401 completes
  via the broker. Same method as the verified mechanism, run through the real
  broker.
- A2 A request from another uid gets 403. A request after the canonical
  credential expired gets 401 with a message naming think2. With think2
  unreachable it gets 503.
- A3 `nix eval` tests and the Python tests pass. `cargo test -p executors`
  passes, and `pnpm run format` / lint are clean for vibe-kanban.
- A4 After deploy: the broker is active on the workers, the worker environment
  carries the override, think2 has neither.
