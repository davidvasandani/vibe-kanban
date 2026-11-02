# Feature specification: wake-ups survive completion

Status: specified. Applied `/speckit.specify` using the repository command's template, scoped to this task rather than its stale browser-scrolling path.

An agent finishing merged work can arrange a later follow-up without asking the operator to return. An operator can reopen completed work without the archive reconciler immediately undoing that choice.

## Requirements and acceptance
- FR-1: All automatic archive sources respect active turns, running pollers, pending and dispatching wake-ups. Holds release after work completes or is cancelled.
- FR-2: Explicit unarchive suppresses previously observed completion intent; only a new completion event can supersede it. Concurrent stale attempts cannot win.
- FR-3: Valid scheduling reactivates archived work atomically with accepting the timer. Missing/deleted worktrees remain rejected with actionable provenance. Invalid schedules do not reactivate work.
- FR-4: Archive provenance is persisted and visible to operators and MCP callers.
- FR-5: Agent sessions can schedule with only a deadline/delay and prompt; identity is exact and supplied by the execution environment. External clients remain explicit.
- FR-6: Follow-ups retain the stored prompt, survive restart, and wait behind active turns using existing dispatch admission.

## Scope
Vibe Kanban only. No unrelated service changes, new timer engine, or changes to timer limits. Manual archive remains intentional and may stop work.

## Open questions
Resolved in clarify: use atomic schedule-and-reactivate; retain completion intent separately; never infer a session from most-recent activity.

## Clarification record
Applied `/speckit.clarify`. Scheduling activates locally without reopening the remote Done issue. Explicit unarchive retains its existing remote reopening behavior. Existing archived records without provenance report unknown legacy origin honestly. Historical incident attribution remains unproven; current code paths are documented. No user decision remains open.
