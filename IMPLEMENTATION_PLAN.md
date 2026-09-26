# Implementation plan: vk/6c79-codex-auth-error

See `SPEC.md` (requirements H1–H8, V1–V3). Two repos, one branch name
(`vk/6c79-codex-auth-error`) in each.

## Part A — homelab (primary fix)

A1. `modules/codex-refresh-broker.py` (new, Python stdlib only)
- Pure helpers, so they can be unit tested:
  - `access_token_exp(auth)` decodes the JWT payload, padded base64url. Returns
    `None` on any failure and never raises with token text.
  - `socket_owner_uid(client_ip, client_port, server_port, proc_root)` parses
    `/proc/net/tcp` and `/proc/net/tcp6` for the connection whose local end is
    the client's (ip, port) and remote end is the server port. Returns its uid.
  - `build_token_response(auth, now, min_remaining)` returns
    `(status, body_dict)`:
    - 200 `{id_token?, access_token}` (never `refresh_token`);
    - 503 `{"error": {"code": "cluster_credential_expired", "message": …think2…}}`
      when the token is expired or unreadable. Never 401, and never a code
      Codex recognises (research R3).
  - `Puller`: single-flight plus a minimum interval around a `pull()` callable.
    Concurrent callers share one run. A run that succeeded within
    `min_interval` seconds is reused. Failures aren't cached, so every waiter
    of the failed run sees the failure.
- `Handler(BaseHTTPRequestHandler)`:
  - POST `/oauth/token`: uid gate (403), bounded JSON body (≤ 64 KiB),
    `grant_type` check (400), `Puller.run()` (503 on failure), then read
    `auth.json` and `build_token_response`.
  - POST `/oauth/revoke`: 200 `{}`, no-op.
  - Everything else: 404/405.
  - Logs one line per request: path, status, caller uid, seconds remaining.
    No tokens.
- `main()`: argparse with `--listen`, `--port`, `--codex-home`, `--sync-command`,
  `--min-interval`, `--min-remaining`, `--sync-timeout`. Runs a
  `ThreadingHTTPServer` bound to 127.0.0.1.

A2. `modules/codex-auth-sync.nix`
- New options under `services.codexAuthSync.refreshBroker`:
  - `enable` defaults to `!isCanonicalSource`;
  - `port` defaults to 18219;
  - `minInterval` = 30, `minRemaining` = 300, `syncTimeout` = 60;
  - read-only `url`, which is null when disabled.
- `systemd.services.codex-refresh-broker`:
  - `wantedBy multi-user.target`, `after network-online`;
  - `Restart=always`;
  - same user, hardening and ReadWritePaths as the sync unit, plus
    `IPAddressAllow=localhost` / `IPAddressDeny=any` is **not** applied, because
    the pull uses SSH to the source;
  - ExecStart runs python3 with the script and flags, and `--sync-command` set
    to `lib.getExe cfg.package`;
  - environment `CODEX_HOME`, `HOME`.
- Export `CODEX_REFRESH_TOKEN_URL_OVERRIDE` through an owner-scoped `environment.extraInit` export (login shells of the credential's own account only) when enabled. Other users keep their own refresh path (review finding).
- Rewrite the "fails harmlessly" paragraph in the sync script's comment so it
  describes the broker path accurately.

A3. `modules/vibe-kanban-rebuild.nix`
- Worker service `environment //= { CODEX_REFRESH_TOKEN_URL_OVERRIDE = url; }`
  when `config.services ? codexAuthSync` and the url isn't null. No dependency
  edges.

A4. Tests
- `tests/codex-refresh-broker.py` (unittest): helpers, plus an end-to-end
  handler test against a real `ThreadingHTTPServer` on an ephemeral port. Uses
  a fake sync command (a Python callable injected), a temp `CODEX_HOME`, and an
  injectable uid resolver.
- `tests/codex-auth-sync-module.nix`: assert the broker service and env exist on
  a worker and are absent on think2; assert the `url` format.
- `tests/codex-auth-freshness-isolation.nix` (or a sibling): assert no unit in
  the real `nixosConfigurations` has Requires/BindsTo/PartOf/Requisite on
  `codex-refresh-broker.service`, with the existing positive control.
- `.github/workflows/deploy-invariants.yml`: run the new Python test.

A5. Verify: `nixfmt --check`, `nix eval` both tests (after committing), the
Python tests, and a live scratch run of the real broker script on think5 with a
corrupted token (acceptance A1/A2).

## Part B — vibe-kanban

B1. `crates/executors/src/executors/codex/normalize_logs.rs`
- `fn refresh_failure_setup_message(message: &str) -> Option<String>` matches
  "Failed to refresh token", or "refresh_token" together with
  "invalid"/"empty"/"expired"/"reused". It returns the actionable text,
  including the original message.
- Use it in the v2 `ServerNotification::Error` arm and the legacy
  `EventMsg::Error` arm. On a match: `SetupRequired` plus that text. Otherwise
  unchanged.

B2. `crates/executors/src/executors/codex/auth_refresh.rs`
- `fn refresh_is_possible(auth_path, override_present) -> bool` is false only
  when a ChatGPT credential has an empty `refresh_token` and there's no
  override.
- In `refresh_credentials_if_stale`, after the stale check:
  `if !refresh_is_possible(..) { warn once; return; }`.

B3. Unit tests next to both, then `cargo test -p executors`, `pnpm run format`,
and `cargo clippy -p executors`.

## Part C — ship

- Independent review of both diffs.
- Update the knowledge base in both repos.
- Open and merge a PR in each repo, homelab first. The VK change is independent
  and fail-safe.
- After comin deploys, verify A4 on the nodes.
