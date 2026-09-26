# Prior knowledge: `vk/7e4f-auto-error-remed` follow-up (similar-issue reuse)

## auto-error-remediation.md (written by this task's first round)

- All guards reserve their slot before I/O and fail closed. A lookup that
  gates spawning must follow the same rule: if it cannot answer, do not
  launch.
- The durable recursion guard is the `Auto-fix: ` name prefix. The issue
  carries a `<!-- vk:auto-remediation … -->` marker. That marker is the
  natural key for finding earlier remediation issues.
- The in-memory 24 h dedupe is per *source workspace* only. It cannot see
  across workspaces or restarts, which is the gap this follow-up closes.

## issue-workspace-lifecycle.md / remote `db/issues.rs`

- "Active" is decided by status name, case-insensitive and project-scoped:
  `lower(name) NOT IN ('done','cancelled','canceled')`. That exact rule is
  already used in `crates/remote/src/db/issues.rs`. Reuse it rather than
  `completed_at`, which not every terminal status sets.
- The remote `search` is `ILIKE '%…%'` over title and description, with LIKE
  metacharacters escaped. Searching for the marker text is exact and safe.

## issue-workspace-advisory.md

- Identity comes from ids and markers, not names. Titles are copied and
  collide. Match on the marker and on error evidence, never on the
  `Auto-fix: …` title.
