# Verification: Vibe Kanban Soft Restarts

## Automated checks

| Check | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | Passed |
| `pnpm run format` | Passed; Rust and all frontend packages formatted |
| `cargo test -p worker --lib` | Passed: 48 tests |
| `cargo check -p worker --bin vibe-kanban-worker` | Passed without warnings |
| `pnpm --filter web-core test` | Passed: 28 files, 240 tests |
| `pnpm --filter web-core run check` | Passed |
| `pnpm --filter web-core run lint` | Passed |
| `git diff --check` (Vibe Kanban and homelab) | Passed |
| `nix-instantiate --parse modules/vibe-kanban-rebuild.nix` | Passed |
| `nix eval .#nixosConfigurations.think2.config.systemd.services.vibe-kanban-worker-distribute.script --raw` | Passed (expected dirty-tree warning only) |

## Covered behavior

- Worker execution continues and retains ordered output while coordinator event polling is absent.
- Admission drain refuses genuinely new dispatch while preserving same-ID/same-digest idempotent retry.
- Drain safety requires acknowledged admission closure plus zero active executions.
- Worker candidate starts drained through the persisted marker and reopens only after deployment health success/rollback.
- Workspace JSON-patch state remains initialized/rendered across same-endpoint reconnect and retries with bounded jitter.
- Restart status is hidden for first load, visible for a post-load disconnect, and clears on recovery.

## External rollout drill

Not executed from this development workspace because deploying to/restarting the production Vibe Kanban hosts is an external state change beyond repository implementation. The first rollout has an intentional bootstrap gate: existing workers that do not yet expose `admission_draining` are deferred with “one-time manual idle activation required.” An operator must confirm each worker is idle, activate this release once, then subsequent distributions use the race-free automatic drain protocol.

Recommended first drill after bootstrap:

1. Start a long-running coding-agent turn on a worker and note execution/process identity.
2. deploy/restart only the coordinator; confirm the browser retains content, shows reconnecting, and the same turn continues/output replays;
3. trigger worker distribution while the turn runs; confirm activation is deferred and the turn continues;
4. let all worker executions reach zero and rerun distribution; confirm drain acknowledgement, release activation, health success, and admission resume;
5. test a deliberately unhealthy worker candidate in staging and confirm rollback plus admission resume.

## Review and knowledge

Independent Codex reviews of both repository commits report no significant
findings after confirmed issues were fixed. Knowledge-base capture remains for
stage 12.
