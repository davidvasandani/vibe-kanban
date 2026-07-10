# Project Knowledge Base

Distilled, reusable knowledge about this codebase, written by and for task
agents (and humans). Each page covers one topic and lists the task ids that
contributed to it.

## Conventions

- One topic per page, kebab-case filename, `# Title` heading.
- Every page ends with a `## Contributed by` section listing task ids
  (e.g. `vk/de6e-improve-column-s`) so knowledge can be traced to its task.
- Update an existing page rather than creating a near-duplicate; add your
  task id when you do.
- Keep this index in sync: one line per page.
- Record only knowledge that is reusable beyond the task that produced it —
  architecture decisions, non-obvious gotchas, rejected alternatives and why.
  Plain code structure belongs in `CLAUDE.md`/`AGENTS.md`, not here.

## Pages

- [external-connector-sync.md](external-connector-sync.md) — Connecting the
  remote server to an outside system (shipped for Jira): why connectors live
  in `crates/remote` (not the local SQLite model), the stored-credential
  destination-pinning rule (atomic with the write, or TOCTOU reopens it),
  lease-token pass claiming, echo-free per-field 3-way merge with
  same-transaction snapshots + post-write re-read, config-cascade link
  deletion, and Jira API / prepare-db / single-user-E2E gotchas.
- [electric-sync-fallback.md](electric-sync-fallback.md) — Client Electric
  hybrid sync + REST fallback: Electric-first-then-fallback architecture, the
  error/recovery callback chain to the navbar banner, why the readiness timeout
  must fall back silently ("falling back is recovery, not an error"), and the
  stable-callback / caching / testing gotchas.
- [kanban-issue-panel-sections.md](kanban-issue-panel-sections.md) — The
  issue detail/create panel (`KanbanIssuePanel.tsx`): section order is owned
  by the `packages/ui` component (containers only supply render props), the
  edit-mode section guard, the one-separator border convention (flip
  `border-t`/`border-b` when moving a section across the title/description
  block), and the rendered-DOM order-test recipe incl. the
  `NODE_ENV=production` act() gotcha.
- [kanban-items-state-and-activity-grouping.md](kanban-items-state-and-activity-grouping.md)
  — The `items` array ↔ drag-and-drop index/sort_order contract, the
  `isSyncingRef` rebuild-swallowing gotcha, the In progress Active/Waiting
  split, preference-gated vs semantic workspace signals, name-based
  "In progress" identification.
- [mobile-kanban-scrolling.md](mobile-kanban-scrolling.md) — Mobile kanban
  board scroll/snap architecture: nested single-axis scrollers, the
  `overflow-y` → `overflow-x` promotion gotcha, gesture routing, history of
  fixes.
- [self-hosted-deployment.md](self-hosted-deployment.md) — Versioned-release
  deploy contract (`VK_RELEASES_DIR`), why services must not run from the
  source checkout, deploy-loop invariants (reconciler over edge triggers,
  health-gated rollback, paging), health endpoints, rejected alternatives.
- [project-context-map.md](project-context-map.md) — Giving a spawned issue its
  scope in a monorepo: a machine-readable `project-context.json` mapping service
  → source path → governing IaC (JSON+jq not YAML, empty-list = no IaC, single
  source of truth via a docs pointer), and the CI path-existence check plus the
  false-pass gotchas reviewers exploit (empty-path skip, pipe-subshell exit
  code, additionalProperties parity at every level).
- [appbar-rail-and-org-tiles.md](appbar-rail-and-org-tiles.md) — The left
  AppBar rail: its slots/sections, the reusable icon-tile recipe (40×40,
  initials, inline-`hsl` active state, right tooltip), and the in-rail org
  switcher (`AppBarOrgTile`) — client-derived org color (no `color` field),
  persisted expand state (`useOrgRailStore`), and two component gotchas
  (optional-controlled state needs an internal fallback; don't render a
  no-op `<button>` — use a non-interactive tile).
- [family-os-sandbox-stack.md](family-os-sandbox-stack.md) — Live Family OS
  backend in VK sandboxes (`homelab/apps/family-os/scripts/sandbox.sh`):
  sandbox environment facts (no cgo, Postgres binaries on PATH,
  `NODE_ENV=production` skipping devDeps, `core.fileMode=false`), why dev
  seed rows go into the already-applied `0099_seed` (version-tracked, no
  checksums), the Google-API local-stub pattern (`WithEndpoint` +
  `WithoutAuthentication`, marshal real drive/v3 structs, test through the
  real client), `familyctl` token parsing + fixed seed UUIDs, and
  provisioning-script invariants (restart app procs every `up`, comm-check
  pidfiles, probe data not process liveness).
