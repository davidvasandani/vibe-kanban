# Validation: Server Metrics Low-Disk Warnings

Validated on 2026-08-13:

- `pnpm install --frozen-lockfile` — passed.
- `pnpm run generate-types` — passed; `shared/types.ts` regenerated.
- `pnpm run remote:generate-types` — passed; `shared/remote-types.ts` regenerated.
- Focused Vitest suites for disk classification, Server Metrics container,
  collapsed header, and RightSidebar — 19 tests passed.
- `pnpm --filter web-core run check` — passed.
- `cargo check -p server -p node-metrics -p services` — passed.
- `cargo check --manifest-path crates/remote/Cargo.toml` — passed.
- `cargo test -p node-metrics disk_alert_threshold_tests` — 2 passed.
- `cargo test --manifest-path crates/remote/Cargo.toml low_disk_tests --lib`
  — 1 passed.
- `nix eval --json .#nixosConfigurations.think2.config.services.vibe-kanban-rebuild.diskAlerts`
  — passed and returned the documented 10%/5 GiB warning and 2%/1 GiB
  critical defaults.

The remote Postgres resolve-or-create path is compile checked and its canonical
body is unit tested. Its transaction-scoped advisory lock is the concurrency
guard; this workspace does not provide a disposable configured Postgres instance
for a live concurrent route test.
