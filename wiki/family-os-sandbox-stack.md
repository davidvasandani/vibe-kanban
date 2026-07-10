# Family OS sandbox stack (live backend in agent worktrees)

How to get a live Family OS backend (Postgres + brain + Drive stand-in) inside
a VK sandbox, and the non-obvious constraints that shaped it. The user-facing
guide is `homelab/apps/family-os/AGENTS.md`; this page records what a future
task needs to know before extending or debugging the setup.

## One command

`homelab/apps/family-os/scripts/sandbox.sh up` — idempotent, ~15s from clean,
exits 0 only after authenticated data probes pass (`/readyz`,
`/v1/search?q=passport`, `/v1/drive/files`, `/v1/tasks` all non-empty). State
in gitignored `apps/family-os/.sandbox/`; `down`/`status`/`reset` complete the
lifecycle. Token for seeded owner Alex is persisted at `.sandbox/token`;
`.sandbox/env` is sourceable and exports `VITE_API_BASE`/`VITE_API_TOKEN` for
the web app.

## Sandbox environment facts (verified, will bite you)

- **No C compiler** — plain `go build` fails at `runtime/cgo`. Everything must
  be `CGO_ENABLED=0` (nothing in the brain needs cgo; the DuckLake client is
  not the cgo duckdb driver).
- **Postgres server binaries are on PATH** (16.x via NixOS system profile):
  `initdb`/`pg_ctl` work as the unprivileged sandbox user with
  `listen_addresses=''` + `unix_socket_directories=<private dir>`. No Docker,
  matching production's direct-systemd model. `trust` auth on a chmod-700
  socket dir is the single-user stand-in for prod's peer auth — re-tighten
  the dir on every `up`, not just at creation.
- **The sandbox shell exports `NODE_ENV=production`** — bare
  `npm/pnpm install` silently skips devDependencies (no `vite`, no `tsc`).
  Use `pnpm install --prod=false` or override `NODE_ENV`.
- **`core.fileMode=false` is set in the homelab clone** — `chmod +x` on a new
  script does NOT reach the git index; you must
  `git update-index --chmod=+x <file>` or the committed script won't be
  executable for anyone else.

## Design decisions worth reusing

- **Seed data lives in the existing `0099_seed` migration, not a side
  channel.** The brain's migration runner (`internal/migrate`) records
  applied versions in `schema_migrations` *without checksums*, so editing an
  already-applied migration never re-runs anywhere it was applied (prod
  untouched) while fresh databases get everything from `migrate up` alone.
  Corollary: a *new* numbered migration would apply to prod on its next
  deploy — for dev-only rows, editing 0099 is strictly safer.
- **FTS needs no worker**: `source_refs.fts` / `document_chunks.fts` are
  `GENERATED ALWAYS ... STORED`, so plain SQL inserts are immediately
  searchable. Don't drag River workers into dev provisioning.
- **Google-API stand-in pattern**: point the real generated client at a local
  stub via `option.WithEndpoint(url)` + `option.WithoutAuthentication()`
  (env-gated: `FAMILYOS_DRIVE_ENDPOINT`, empty = prod behavior). Two gotchas:
  `option.WithScopes` conflicts with `WithoutAuthentication` (scopes imply a
  credential lookup) — only set scopes on the credentialed path; and test the
  stub *through the real client* (httptest), which pins URL paths and the
  wire format (e.g. int64 `size` marshals as a JSON string — emit responses
  by marshaling `google.golang.org/api/drive/v3` structs, never hand-written
  JSON). The stub 404s must use Google's `{"error":{"code":...}}` envelope so
  `googleapi.Error` mapping works.
- **`familyctl` token plaintext is parseable**: printed once inside a box —
  the line is `  │  <token>`, so `awk '$1=="│"{print $2}'` extracts it. The
  seeded people have fixed UUIDs (Alex owner `…10`, Sam adult `…11`, household
  `…01`); Sam lacks the `health` domain, so `search?q=pediatrician` as Sam
  returning 0 hits is the ready-made deny-by-default check.
- **Provisioning scripts: restart app processes on every `up`.** Binaries are
  rebuilt each run; verifying probes against a still-running old process
  falsely blesses code that was never loaded (Codex review catch). Converge
  only the stateful parts (pgdata, token). Also validate pidfiles with
  `ps -o comm=` before killing — a reused PID must be treated as stale, not
  killed.

## Contributed by

- vk/c58a-provision-a-live
