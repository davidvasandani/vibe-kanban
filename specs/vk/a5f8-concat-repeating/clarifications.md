# Clarifications: Background Workspace Creation

## Resolved Decisions

### Restart reconciliation is conservative

The accepted job persists its state, but this feature does not attempt arbitrary mid-phase replay after a coordinator restart. On startup, every unfinished creation becomes a visible interrupted/failed creation with guidance to create a replacement workspace. Even an execution row is not sufficient completion evidence because a crash can occur after inserting it but before process startup returns. This guarantees that accepted work never remains pending forever while avoiding unsafe duplicate Git or execution work.

Reason: the immediate defect is browser-request cancellation. Exact phase-resume would require making every existing repository, remote import, worktree, placement, and execution operation replay-safe and substantially expands scope. A persisted truthful interruption meets the durability and observability contract safely.

### The existing endpoint becomes asynchronous

`POST /api/workspaces/start` remains the single create-and-start operation, but its success response becomes an acceptance response containing the workspace and creation status rather than a completed execution process. All in-repository callers migrate together. No parallel synchronous endpoint is retained.

Reason: retaining a synchronous path leaves non-browser callers vulnerable to the same request-lifetime bug and creates two semantics for one product action.

### Failed creation is informational in this increment

A failed workspace shows the persisted error and directs the user to create a new workspace. It does not expose an in-place Retry button.

Reason: safe in-place retry requires phase-specific replay semantics. Users still receive an actionable, terminal state instead of an indefinite spinner, and can resubmit from the create flow.

## Remaining Open Questions

None.
