# Validation: Search by Issue ID

Validated on 2026-09-16.

- `pnpm install --frozen-lockfile`: passed in the fresh worktree.
- Focused Vitest (`globalSearch.test.ts`, `GlobalSearchDialog.test.tsx`): 11
  tests passed across two files.
- `python3 scripts/test-global-search-postgres.py` with PostgreSQL 17.10 from
  the Nix store: passed membership, cross-organization, issue-ID case folding,
  inaccessible issue, literal-input, ownership, archive, and category-bound
  assertions against the production SQL.
- `SQLX_OFFLINE=true cargo check --manifest-path crates/remote/Cargo.toml`:
  passed.
- `pnpm run format`: passed for all configured Rust and frontend workspaces.
- `pnpm --filter @vibe/web-core run lint`: passed.
- `pnpm run check`: passed, including both frontends, web-core, UI, the primary
  Rust workspace, and the remote Rust workspace.
- `git diff --check`: passed.

No schema migration, generated contract, dependency, deployment configuration,
or non-Vibe-Kanban service changed.
