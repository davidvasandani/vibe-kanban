# Prior Knowledge: Workspace Server Affinity (`9a64-vk-workspace-aff`)

The project knowledge base is populated. Both `docs/knowledge-base/` and the older `wiki/` were searched read-only before planning.

## Relevant pages

| Page | Reusable guidance |
| --- | --- |
| `docs/knowledge-base/clustered-workspace-execution.md` | Coordinator owns placement and user-facing state; persist affinity rather than inferring it; workers own their processes; dispatch is idempotent; stale/offline state is indeterminate; shared worktrees are portable only because repository and worktree paths are common across nodes. |
| `docs/knowledge-base/interrupted-worktree-recovery.md` | Stop and preservation are separate responsibilities; failures can leave a running row intentionally for recovery; partial success must be reported truthfully. |
| `wiki/agent-process-lifecycle.md` | One turn maps to one execution process and execution-scoped streaming state; a restart must create a new turn/execution rather than trying to move or reuse a live process. |
| `wiki/workspace-carousel-view.md` | Explicit workspace-id queries are route-independent; avoid mounting extra workspace providers; workspace status semantics include interrupted runs; list-level rendering must avoid multiplying subscriptions and requests. |
| `wiki/kanban-items-state-and-activity-grouping.md` | Activity must derive from the canonical workspace execution signal, not a display-preference-gated projection; derived UI state must not alter persisted ordering. |
| `wiki/kanban-issue-panel-sections.md` | Panel section order belongs in the presentational panel; component tests should assert rendered ordering and use the package test script because the shell may export production `NODE_ENV`. |
| `wiki/create-mode-repo-branch-defaulting.md` | The create-mode chat and issue-create panel are distinct surfaces; use the actual shared seam rather than a similarly named dormant component. |

## Constraints carried into the spec and plan

1. The coordinator remains the sole authority for affinity/placement, execution state, and migration orchestration. The browser must not implement stop → update → restart as separate best-effort calls.
2. Never migrate a live process between workers. Stop the current execution and create exactly one new follow-up execution whose dispatch uses the new affinity.
3. Preserve idempotency and serialize migration requests. Lost responses or duplicate confirmation clicks must not produce duplicate restarted agents.
4. Treat an unreachable worker as unknown/indeterminate, not idle. Do not claim a migration is safe solely because worker telemetry vanished.
5. A stop failure must not be papered over by changing affinity. A restart failure after a successful reassignment is a distinct partial-success result and the stopped workspace remains recoverable.
6. Do not infer affinity from the selected UI host. Read the persisted workspace placement/requested worker data and resolve names against the worker inventory.
7. An explicit worker must pass current scheduler eligibility (online, healthy shared mount, not draining). The backend repeats this check because client inventory can be stale.
8. Shared worktree portability has already been established by the cluster implementation; this feature changes execution ownership, not filesystem layout or Git worktree administration.
9. List-level UI must avoid one placement/worker request or websocket per row. Extend/batch the workspace summary contract or share cached bulk placement state.
10. The workspace right-drawer section should follow the existing presentational-section ownership and border/accordion conventions. Test the rendered section and behavior through the appropriate package script.
11. Use the canonical `isRunning`/execution-process signal for whether confirmation is required; do not base safety behavior on a hidden preference or drawer-local approximation.
12. Generated TypeScript types come from Rust declarations and must be regenerated, never edited by hand.

## Implications for open questions

- `Automatic placement` is an affinity choice, not a promise to move immediately to a different random worker. Planning must define whether a stopped, already placed workspace is released and re-reserved immediately or retains its current placement until the next dispatch.
- The local coordinator has no `worker_nodes` row, so it needs an explicit product representation if it is selectable; it cannot be treated as another worker UUID.
- Restart should use the session that owns the running execution, because execution lifecycle and logs are session-scoped. If there are several running processes, the backend must reject ambiguity or define deterministic workspace-wide stop semantics.
- Dev servers are persistent executions with a different run reason. Migration must state whether they are stopped/restarted; silently leaving one running on the former server would violate the displayed workspace affinity.
