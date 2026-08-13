# Data Model: Background Workspace Creation

## Workspace additions

`creation_status: WorkspaceCreationStatus`

- `queued`: workspace identity persisted; background consumer not yet claimed.
- `running`: one coordinator consumer claimed the workflow.
- `ready`: materialization and initial execution startup completed.
- `failed`: creation did not complete or was interrupted by coordinator restart.

`creation_error: Option<String>`

- absent for queued, running, and ready;
- bounded, safe user-facing explanation for failed;
- detailed underlying error remains in structured server logs.

## State transitions

```text
new row -> queued -> running -> ready
                       |          ^
                       +-> failed |
queued/running at startup -> failed
```

Invariants:

- Existing pre-feature workspaces migrate to `ready`.
- Only an atomic `queued -> running` update claims work.
- `ready` is written only by the live consumer after initial execution startup returns successfully; an execution row alone is insufficient restart evidence.
- `failed` is terminal for this feature; users create a replacement workspace.
- The workspace ID is the creation-operation identity; no second operation row is needed while retry-in-place is out of scope.
