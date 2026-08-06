# Validation: Codex rollout lineage transfer

## Completed checks

- `pnpm install --frozen-lockfile` — passed.
- `pnpm run generate-types` — passed; `shared/types.ts` includes
  `session_transfer_failed`.
- `pnpm run format` — passed.
- `pnpm --filter @vibe/web-core run check` — passed.
- `pnpm run backend:check` — passed for both the main workspace and the
  separate `crates/remote` manifest.
- `cargo check -p worker -p server` — passed.
- `cargo clippy -p executors -p worker -p server --all-targets -- -D warnings`
  — passed.
- `cargo test -p executors rollout_transfer --no-default-features` — passed
  (3 tests: direct/ancestor transfer and replay; traversal/symlink/conflict;
  checksum/cycle/oversize).
- `cargo test -p worker transfer_routes_reject_operation_and_target_substitution`
  and `transfer_routes_accept_bodies_above_axum_default_limit` — passed.
- `cargo test -p db --lib` — passed (9 tests).

## Deployment validation

The Vibe Kanban worker module now sets `CODEX_HOME` explicitly to
`/var/lib/vibe-kanban/.codex`, creates it as mode `0700` owned by
`vibe-kanban`, and contains matching evaluation assertions.

`nix-instantiate --eval --strict [--impure] tests/vibe-kanban-cluster.nix`
cannot complete in this workspace. Attempts encountered the pre-existing
`invalidRole` assertion while the Nix fetcher cache was unavailable, then an
unavailable registry/source with network resolution disabled. The failure
occurs before the new assertions are evaluated.

## Independent review

- Repeated `codex review --base vk/9a64-vk-workspace-aff` passes drove the
  recovery and containment fixes recorded in `review.md`; the final pass
  reported no significant finding.
- `codex review --commit 1feb2d85` in the homelab repository reported no
  significant finding.
