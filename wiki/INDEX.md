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

- [browser-session-control-arbiter.md](browser-session-control-arbiter.md) —
  Workspace browser sessions with shared human/agent control: the
  three-lock concurrency shape (control mutex / command gate / Arc'd driver
  handle), lease+generation arbitration with invalidate-never-replay,
  in-flight idempotency reservation (typed cached errors, owner-drop
  cleanup), the serde_json `preserve_order` × internally-tagged-f64 gotcha,
  typed errors through the single-message ApiError channel, event-driven
  lease cleanup with TTL backstop, and the CDP driver seam
  (`BROWSER_UNAVAILABLE` degradation, fire-and-forget screencast acks).
- [bundled-file-seed-manifests.md](bundled-file-seed-manifests.md) —
  Incrementally delivering new user-editable bundled defaults without
  resurrecting deletions or overwriting edits: seen-filename manifests, explicit
  legacy baselines, commit-last reconciliation, cross-platform atomic replace,
  and failure/concurrency tests.
- [managed-cli-tool-catalog.md](managed-cli-tool-catalog.md) — How to extend
  the app-managed CLI catalog: stable wire ids, complete catalog registration,
  immutable artifact URLs and SHA-256 pins, per-platform archive executable
  paths, generated TypeScript types, generic route/UI behavior, focused
  validation, and host-first PATH propagation across local and clustered
  workspace process boundaries.
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
  reap, the env gate, the Codex/ACP Phase-3 decisions, and why cleanup-skip early
  finalization must dispatch queued follow-ups before setting its finalized
  guard.
- [slack-shortcut-ai-summarization.md](slack-shortcut-ai-summarization.md) —
  Optional AI thread summarization for the Slack "Create issue from message"
  shortcut: the ack-fast/enrich-later shape (all slow work in the post-ack
  spawned task, `views.open` returns the view id, single `views.update` swap),
  the mid-edit race + "✨ Summarizing…" hint mitigation, the outbound LLM call
  (raw reqwest, Jira-style HTTP-status errors not Slack's `ok` envelope,
  `output_config.format` for `{title,description}`, `maxLength` unsupported →
  prompt+post-truncate), degrade-to-mechanical on every failure, write-only
  encrypted key + no-key/transcript-in-logs hygiene, the all-three effective
  gate, and the non-retroactive `*:history` scopes needing a re-install.
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
  `useRepoBranchSelection`/`RepoBranchSelector` stack with divergent defaults —
  plus the contract this places on every *backend* consumer of `target_branch`:
  resolve local-then-remote, never normalise the prefix away.
- [kanban-issue-panel-sections.md](kanban-issue-panel-sections.md) — The
  issue detail/create panel (`KanbanIssuePanel.tsx`): section order is owned
  by the `packages/ui` component (containers only supply render props), the
  edit-mode section guard, the one-separator border convention (flip
  `border-t`/`border-b` when moving a section across the title/description
  block), the rendered-DOM order-test recipe incl. the
  `NODE_ENV=production` act() gotcha, and how an external-connector link
  (Jira badge) is surfaced identically on the card and the panel header (one
  `JiraBadge` + `jiraLink` data prop + `getJiraLinkForIssue` lookup).
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
  health-gated rollback, paging), one-source artifact identity (`sha` plus an
  optional build/publish timestamp surfaced through `/api/info`), health
  endpoints, rejected alternatives.
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
- [workspace-navbar-breadcrumbs.md](workspace-navbar-breadcrumbs.md) — How
  workspace breadcrumbs preserve linked issue identity across asynchronous
  shape states: relationship truth vs loaded rows, cross-shape cursor races,
  authoritative issue-detail fallback, `simple_id` vs UUID, the
  none/loading/resolved/unavailable state model, and the pure-builder testing
  seam.
- [workspace-carousel-view.md](workspace-carousel-view.md) — Rendering N
  live workspace chats at once (the carousel view): the prop-driven chat
  stack (`ExecutionProcessesProvider` + `WorkspacesMainContainer`, no
  per-instance `WorkspaceProvider` — markSeen-on-mount / global diff-store /
  websocket hazards), the chat-editor-autofocus gotcha (focus is not a user
  signal; key markSeen and order-freeze off pointer/keydown), the
  starvation-safe debounced re-sort, needs-feedback tiering incl.
  `interrupted`, and per-column error boundaries.
- [workspace-context-bar-responsive-visibility.md](workspace-context-bar-responsive-visibility.md)
  — Why the floating workspace context bar is desktop-only, how responsive
  layout state and physical-device detection combine as a visibility truth
  table, and where to keep the policy without changing desktop drag/snap
  behavior or the presentational UI component.
- [flexible-collapsible-panel-stacks.md](flexible-collapsible-panel-stacks.md)
  — How bounded vertical panel stacks let expanded collapsibles share remaining
  height: expansion-owned flex participation, the complete `min-h-0` chain,
  content-scroll ownership, the outer short-window header-scroll fallback, and
  desktop-only fixed chrome in a drawer component also reused on mobile.
