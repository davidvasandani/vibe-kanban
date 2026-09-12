# Request-independent workspace creation

Tags: `vk/5e1e-vk-workspace-cre`

## Accept before slow lifecycle work

Workspace creation can spend many seconds associating repositories, importing issue context, selecting a worker, materializing worktrees, and starting the first agent. If an Axum handler awaits that entire sequence, dropping the browser request can drop the server future at any await.

Persist the workspace identity and an observable `queued` state first, then return it to the client. A Tokio-owned task atomically claims `queued -> running` and performs the existing coordinator-owned workflow. The frontend navigates on acceptance and reads creation state from the ordinary workspace model, so unmounting the create form cannot strand the operation.

## The workspace is the operation identity

A one-time workspace lifecycle does not need a general job framework. The workspace ID plus a compare-and-set status transition gives one authoritative consumer:

- existing workspaces and non-background creation default to `ready`;
- background acceptance changes the new row to `queued`;
- exactly one consumer can claim `queued -> running`;
- only successful return from initial execution startup writes `ready`;
- any workflow error writes a bounded, safe `failed` message while detailed context stays in logs.

Status writes use bounded retries so a transient SQLite failure does not leave the UI spinning. If the database remains unavailable, normal service health is already impaired and startup reconciliation remains the final durable backstop.

## Restart evidence must prove the whole phase

An execution row is not proof that creation completed. Process startup can insert session/execution state and then fail or be interrupted before returning. On coordinator startup, every workspace still marked `queued` or `running` is therefore conservatively marked failed. Do not promote unfinished creation to ready from partial artifacts.

This deliberately avoids replaying repository association, placement, worktree creation, or process startup until those phases have their own durable idempotency contract. A visible interrupted failure is safer than duplicate agents or destructive worktree replay.

## UI convergence

Creation status belongs on the workspace read model used by list/detail streams. Pending workspace detail queries may poll briefly as a fallback; normal database event streams carry the terminal transition. Content that assumes repositories or sessions exist must be guarded above those hooks/components and replaced with pending or failed presentation until the workspace is ready.
