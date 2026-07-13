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

- [agent-process-lifecycle.md](agent-process-lifecycle.md) — How a coding-agent
  turn ends at the process level: the one-turn-one-`ExecutionProcess` identity
  chain, the implicit app-server marker (`exit_signal: Some` vs `None`, distinct
  from `is_persistent()`), the exit monitor's **two** kill points (exit-signal
  `killpg` + tail `start_kill`/`kill_on_drop`), the 250ms OS-exit-watcher
  poll-loop gotcha, why teardown skips `Completed` executions (warm-process
  trap), the pgid re-adoption substrate, the keep-warm enablement order, **and
  the Phase-2 warm registry** — the async `base_url` surfacing over a oneshot,
  park-before-finalization + insert-before-remove, `Arc::try_unwrap` ownership
  (and why the idle sweep must not clone child handles), generation-conditional
  reap, the env gate, and the Codex/ACP Phase-3 decisions.
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
- [task-pipeline-block.md](task-pipeline-block.md) — Per-task pipelines as
  a generated `## Pipeline` block in the issue description: the
  compose/parse round-trip contract, why under-recognizing the selection is
  destructive on recompose (duplicate names scored by block stages, greedy
  heading segmentation), the strict-regex rule for the destructive legacy
  strip path, uncontrolled seeding + remount-by-key, incremental tick
  adjustment on toggle (never a selection-watching reseed effect), and the
  edit-mode "Update Issue" apply rules (cancel debounce, latest-description
  ref, pending-attachments guard).
- [create-mode-repo-branch-defaulting.md](create-mode-repo-branch-defaulting.md)
  — How the create-issue screen ("Which repositories…") picks a repo's target
  branch: the single `addRepoWithBranchSelection` seam (vs the separate
  "Change branch" modal path), the `resolveDefaultBranch` fallback order
  (configured default → `origin/main` → `origin/master` → current → first),
  and the gotchas — remote-prefixed branch names (`origin/main`, not `main`),
  `get_all_branches` sorting current-first (so `branches[0]` ≠ mainline),
  NULL-at-registration `default_target_branch`, and the dormant importer-less
  `useRepoBranchSelection`/`RepoBranchSelector` stack with divergent defaults.
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
