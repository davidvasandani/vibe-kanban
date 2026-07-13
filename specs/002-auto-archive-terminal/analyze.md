# Analysis — spec / plan / tasks cross-check (vk/2f63-auto-archive-wor)

Checked `spec.md`, `plan.md`, `data-model.md`, `tasks.md` against
`.specify/memory/constitution.md`.

## Coverage: every FR maps to a task
| FR | Covered by |
| --- | --- |
| FR-1 terminal statuses {Done,Cancelled} by name | T001 + T002 |
| FR-2 archive atomic with status write (txid) | T002/T003 (runs on tx conn, no new tx) |
| FR-3 no fire on no-change / non-terminal | T002 (status-changed guard + `terminal_status_name` None) |
| FR-4 Done-only unmerged-PR warning | T002 (gate behind `terminal == "Done"`) |
| FR-5 no active workspaces → no-op | T002 (empty early-return, preserved) |
| FR-6 single + bulk paths | T003 (both call sites) |
| FR-7 archive/status failure fails tx; PR-load failure degrades | T002 (map_err on archive/list; `unwrap_or_else` on PR load, preserved) |
| Unit test | T004; run by T005 |
| Tooling gates | T005/T006/T007 |

No FR is left without a task; no task is orphaned from an FR.

## Findings

- **info (spec/plan)** — Terminal-status matching is by name only, so a project
  that renames "Done"/"Cancelled" won't trigger. Explicitly Out of Scope and
  consistent with the existing Done hook + Constitution V (identify by name).
  Accepted limitation, not a gap.
- **info (spec)** — No un-archive on reopen (terminal → non-terminal). Declared
  Out of Scope; the existing Done feature has no un-archive either. Consistent.
- **info (tasks)** — T001–T004 all edit one file and are correctly **not** marked
  `[P]`; only the independent tooling checks are `[P]`. Matches the plan.
- **info (constitution II)** — Full archive-path behaviour needs Postgres, so the
  automated test is the pure `terminal_status_name` decision logic (the part that
  regresses); the DB-side archive reuses already-shipped, unchanged helpers. This
  is the same test depth the constitution's "unit test where feasible" intends;
  acceptance criteria carry the DB-level expectations. No violation.

## Constitution violations
None. The plan's Constitution check (I, II, III, V, VI) holds; IV (shared-UI
boundaries) is N/A — no frontend change.

## Verdict
Consistent and complete. Cleared to implement.
