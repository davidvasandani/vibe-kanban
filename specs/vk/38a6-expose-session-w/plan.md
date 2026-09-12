# Implementation Plan: Recover Wedged MCP Sessions

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Rust/Axum backend with service boundaries in `crates/services` and concrete
  local/cluster orchestration in `crates/local-deployment`.
- The Vibe Kanban stdio MCP server lives in `crates/mcp` and calls backend HTTP
  routes; orchestrator mode carries workspace/session context.
- Executor-owned MCP state is currently available only for Codex through its
  app-server `mcpServerStatus/list` protocol and `McpRefreshControl`.
- Session restart already exists behind
  `POST /api/sessions/{id}/queue/mcp-restart`, backed by a reservation in
  `QueuedMessageService` and consumed by the normal exit monitor.
- Shared API types are authored in Rust and generated with
  `pnpm run generate-types`; `shared/types.ts` is never edited manually.
- React/TypeScript UI lives in `packages/web-core`; MCP settings already include
  connectivity tests and safe Debug issue creation.
- Cluster workers own remote execution processes. Restart operations must route
  through the same affinity and authenticated worker protocol as stop/refresh.

## Architecture & Approach

### One recovery coordinator

Add a session-keyed `McpRecoveryCoordinator` in `crates/services` that owns
complete restart generations and per-server discovery snapshots. It serializes
duplicate requests, publishes atomic old/new vectors, applies one absolute
deadline per generation, and stores only bounded, allow-listed diagnostics.
The coordinator is backend authority for MCP tools and browser polling.

### Session restart reuses the proven handoff

Extract the route-local restart logic in `crates/server/src/routes/sessions/queue.rs`
into a `ContainerService::restart_session_for_mcp` operation. It reuses
`QueuedMessageService` reservation/commit/take semantics, derives the existing
executor profile and continuation prompt server-side, and reaps warm processes.
A running self-call returns `accepted` after committing the reservation; the
exit monitor starts the continuation after the tool response and natural turn
exit. Idle sessions start immediately through the normal follow-up path.

### Workspace restart is a composed backend operation

`restart_workspace_for_mcp` first records a generation, enumerates active
workspace-owned processes, and routes stop/restart to the existing local or
affinity-bound worker owner. Coding-agent resumption uses the session restart
operation. Persistent processes are recreated only when their execution rows
retain a complete launch definition; otherwise the operation records a named
partial failure. Existing stop and startup-recovery preservation paths remain
responsible for dirty Git state.

### Discovery status uses executor evidence

Extend `McpRefreshControl` into a reusable runtime inventory control and retain
it for the current execution. Codex maps its full status response to lifecycle
snapshots, including a distinct successful zero-tool state. Claude Code's
structured `system/init` message already contains the executor's registered
tool-name inventory; capture that startup event at the protocol boundary, group
`mcp__<server>__<tool>` names by configured server identifier, and publish the
fresh process's registry snapshot. Configured servers absent from the completed
init inventory become a named `not_registered` terminal outcome rather than
remaining `connecting`. Other executors without equivalent evidence report a
named `registry_unsupported` result rather than fabricated settings-probe
health.

A bounded observer polls until all servers are terminal or the generation
deadline expires; transient error events increment an attempt counter and append
to a small event ring without moving the deadline. Executor diagnostics that can
be safely mapped to allow-listed codes enrich the outcome; raw stderr and config
values never cross the boundary.

`CLAUDE_CODE` is explicitly unsupported for live `refresh_mcp_tools`: its
executor currently publishes no `McpRefreshSignal`. The existing start boundary
already converts that absence to `unsupported`; add direct regression coverage
and update wording to recommend `restart_session`.

### Runtime UI and issue offer

Expose recovery/status endpoints through a small polling hook in workspace chat
and pass runtime snapshots into the MCP status surface. Settings probe results
retain their “last checked” meaning and are labelled as configuration tests.
Runtime state is labelled as active-session registry state; connected with zero
tools is explicit.

Build issue Markdown with the existing `mcpDebugIssue` helpers and duplicate
guard. Runtime bundle construction is a pure function and includes only safe
coordinator fields. The action appears on terminal failures, deadline failures,
or explicit transport/registry mismatch and never auto-submits.

### MCP tools

Add both tools to `crates/mcp/src/task_server/tools/sessions.rs` (session) and
`workspaces.rs` (workspace), register them in both permitted routers, apply
existing context resolution/scope checks, validate session/workspace ownership,
and return structured recovery results. The backend accepts before process
termination so a target can call its own tool safely.

## Data Model

See `./data-model.md`. Recovery generations are initially process-local, matching
the existing MCP refresh coordinator; execution/session/workspace rows and queued
continuation state remain the durable lifecycle authorities. No migration is
required unless implementation proves persistent launch replay needs additional
fields.

## Contracts

See `./contracts/recovery-api.md` for HTTP and MCP request/response contracts.
New optional snapshot fields use Serde defaults for rolling worker compatibility.

## Research Notes

See `./research.md`. No new dependency is planned.

## Constitution Check

- III/VI: extends restart reservation, refresh coordinator, normal continuation,
  worker affinity, and Debug issue machinery rather than adding parallel owners.
- II: contract tests cover lifecycle handoff, bounded discovery, scoped tools,
  generated types, and rendered UI states.
- XI/XIX: diagnostic observations remain exact within the safe allow-list and
  issue creation remains an explicit read-to-action boundary.
- XII/XVII: one backend coordinator owns each restart generation; the active
  executor confirms capabilities; unsupported protocols remain explicit.
- XV/XVIII: workspace teardown preserves work independently of stop success and
  routes through the persisted process owner.
- No constitution deviation is planned.

## Risks & Dependencies

- Claude Code exposes registered tool names in its structured startup event but
  no live reload/control API. Status capture must occur before the existing log
  normalizer discards the init event, and configured-name matching must handle
  MCP tool-name encoding exactly.
- Recreating arbitrary background helpers after a process loss is safe only if
  complete launch input is durable. Partial workspace restart is preferable to
  guessing commands.
- A process-local recovery coordinator loses presentation history on backend
  restart. Durable execution/queue state still prevents duplicate processes;
  startup reconciliation should mark an unfinished presentation generation
  interrupted if this becomes user-visible.
- Remote workers may roll independently. All added protocol fields must default,
  and missing routes/capabilities must map to `unsupported` rather than success.
