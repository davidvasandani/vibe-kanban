# Contract: Background Workspace Creation

## Request

`POST /api/workspaces/start` retains `CreateAndStartWorkspaceRequest` unchanged.

Validation errors return before any workspace is created.

## Acceptance response

```text
CreateAndStartWorkspaceResponse {
  workspace: Workspace
}

Workspace {
  ...existing fields,
  creation_status: "queued" | "running" | "ready" | "failed",
  creation_error: string | null
}
```

A successful HTTP response means the operation was accepted and owns a durable workspace identity. It does not mean worktree creation or initial execution startup completed.

## Read behavior

Existing workspace list/detail endpoints return creation fields. Clients treat:

- `queued` or `running`: render creation progress and do not assume repositories/sessions exist;
- `ready`: use normal workspace UI and execution queries;
- `failed`: render `creation_error` and a route back to create a replacement workspace.

## Background ownership

- Consumer claim: atomic `queued -> running` by workspace ID.
- Duplicate claim: no-op; it must not start another workflow.
- Success: `running -> ready` only after initial execution startup succeeds.
- Failure: `queued|running -> failed` with bounded safe text.
- Startup: unfinished state becomes ready only with positive initial-execution evidence; otherwise failed.
