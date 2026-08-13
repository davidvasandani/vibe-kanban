# Technical Specification: Stable Workspace Order During Restart

## Problem

The workspace sidebar defaults to sorting by most recently updated workspace.
That ordering currently depends on `latest_process_completed_at`, which arrives
from a separate workspace-summary request after the workspace stream has
initialized. Immediately after a Vibe Kanban restart, every workspace therefore
temporarily has no summary timestamp. The comparator places those missing values
first and falls back to workspace name, so the sidebar shows an incorrect order
and repeatedly reorders as summaries become available.

## Goal

Show a deterministic, useful workspace order as soon as the workspace stream is
available, without waiting for all workspace summaries to be read. Preserve the
existing user-selected sort field, direction, pinning, filtering, pagination,
and archive behavior.

## Proposed Behavior

- When sorting by “Updated”, use the latest completed-process timestamp when it
  is available.
- While that summary value is unavailable, use the workspace record's persisted
  `updated_at` timestamp as the immediate fallback.
- Continue to put pinned workspaces before unpinned workspaces.
- Continue to honor ascending and descending order.
- Put genuinely timestamp-less or invalid records after timestamped records so
  incomplete data cannot take over the top of the list.
- Resolve equal timestamps deterministically by workspace name and then ID.
- Apply the same behavior to active and archived workspace lists.

## Scope

This change is limited to the Vibe Kanban service repository. No other homelab
service or deployment configuration is changed.

## Implementation Constraints

- Keep the sort policy in a small testable helper rather than embedding all
  behavior in the sidebar component.
- Do not change generated shared types.
- Do not introduce a schema or API contract change unless investigation proves
  the existing workspace stream does not contain the required fallback value.
- Preserve the current summary refresh behavior; this task changes how partial
  data is ordered, not how summaries are fetched.

## Acceptance Criteria

1. On first render after a service restart, workspaces are ordered by persisted
   workspace update time instead of alphabetically while summaries load.
2. Once process-summary timestamps arrive, the list reflects those timestamps
   without missing-summary workspaces incorrectly sorting ahead of known recent
   activity.
3. Pinned workspaces remain first for both ascending and descending sorts.
4. Created-time sorting remains unchanged except for deterministic handling of
   missing, invalid, and equal timestamps.
5. Active and archived lists share the same comparator behavior.
6. Automated tests cover missing summaries, summary precedence, sort direction,
   pinning, invalid timestamps, and deterministic ties.
7. Repository formatting and targeted frontend checks pass.

## Risks and Mitigations

- `updated_at` may represent broader workspace mutations than process activity.
  It is used only as a temporary/absent-summary fallback; the existing process
  completion timestamp retains precedence.
- Summary arrival can still refine ordering. This is expected, but the initial
  ordering is meaningful and stable rather than name-based.
- A malformed timestamp could produce unstable comparisons. Timestamp parsing
  treats malformed values as missing and uses deterministic tie-breakers.
