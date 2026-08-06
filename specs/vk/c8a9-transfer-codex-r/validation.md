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
  — passed.
- `cargo test -p db --lib` — passed (9 tests).

## Deployment validation

The Vibe Kanban worker module now sets `CODEX_HOME` explicitly to
`/var/lib/vibe-kanban/.codex`, creates it as mode `0700` owned by
`vibe-kanban`, and contains matching evaluation assertions.

`nix-instantiate --eval --strict [--impure] tests/vibe-kanban-cluster.nix`
cannot complete in this workspace because an existing pinned Nix source-store
path (`9s8bs867wxx3zx7gllsv6a9jqs25zjy6-...-source`) is absent. Both pure and
impure attempts fail at the pre-existing `invalidRole` assertion before
reaching the new assertions.

## Pending final gate

- Independent Codex diff review and any resulting re-verification.
