# Implementation Plan: Refresh Active Remote MCP Snapshots

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

The implementation spans the Rust coordinator/worker boundary, the existing
Codex app-server live-refresh control, and the shared React refresh UI. VAS-356
already defines `McpConfigSnapshot`, coordinator-side settings resolution,
execution-scoped Codex homes, and atomic agent-config writes on the worker. The
current refresh endpoint and result model already implement session-level
generations and truthful UI states, but remote worker execution currently drops
the spawned Codex refresh signal and exposes no rematerialize-and-reload route.

Implementation must be based on the VAS-356 commits now present on
`origin/main`; this task branch was created from an older target, so the branch
must first be brought forward without rewriting user changes.

## Architecture & Approach

1. Extend `crates/cluster-protocol/src/lib.rs` with a bounded, authenticated
   refresh request carrying the target execution ID and a freshly resolved
   `McpConfigSnapshot`, plus a secret-safe worker outcome that distinguishes
   materialization, reload/bootstrap, busy, and unsupported.
2. Extend `crates/services/src/services/cluster/client.rs` and
   `crates/worker/src/worker_api.rs` with a signed execution-scoped refresh
   endpoint. The worker verifies path/body identity through the existing request
   authority rules before touching state.
3. Refactor `crates/worker/src/execution.rs` so a live `WorkerJob` retains:
   the execution-scoped Codex config target, the resolved profile adapter needed
   to replace its MCP section, and the Codex `McpRefreshHandle` emitted by the
   spawned app-server. Add a per-job refresh claim/lock whose acquisition is
   short and whose external work happens outside global job-map locks.
4. On worker refresh, validate executor identity and snapshot size, atomically
   rewrite only `mcp_servers` in that execution's existing `config.toml` using
   `write_coding_agent_mcp_servers_to_path`, and call the retained live Codex
   control only after the write succeeds. Never rebuild or replace the scoped
   home, since doing so could disturb symlinked authentication, skills, history,
   and session state.
5. Refactor the coordinator snapshot resolver in
   `crates/local-deployment/src/container.rs` so dispatch and refresh share one
   authoritative profile/settings path. For a remotely placed session, find the
   active execution and its persisted `ExecutionWorkerJob`, route the fresh
   snapshot to the assigned worker, and translate the worker phase result into
   the existing `McpRefreshCoordinator` generation. Local Codex refresh keeps
   the live-control path and gains materialization only when it has an
   execution-scoped snapshot target.
6. Extend `crates/executors/src/mcp_refresh.rs` only as needed to add explicit
   materialization and reload/bootstrap error categories. Regenerate shared
   TypeScript types rather than editing generated files.
7. Update `SessionChatBoxContainer.tsx` and its tests only where new categories
   require distinct, truthful copy; retain the existing pending/refreshed/
   partial/busy/unsupported/failed status model.
8. Add focused worker unit/integration tests for isolation, atomic section-only
   replacement, disabled/removed definitions, contention, and phase-safe
   errors. Add an offline smoke fixture that initializes a deterministic MCP
   server and executes `tools/list` using the refreshed scoped `CODEX_HOME`.
9. Add coordinator regression coverage for snapshot A -> settings B -> worker
   refresh -> status confirmation without changing session/thread identity.

## Data Model

See `./data-model.md`. No database migration is planned; the worker retains
ephemeral live refresh state on the already in-memory job object, while
placement and execution-to-worker affinity continue to come from existing
tables.

## Contracts

See `./contracts.md` for the coordinator/worker refresh request and outcome
contract and the ordering guarantees around materialization and Codex reload.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- VI and XXI: dispatch and refresh share the existing settings/profile resolver
  and MCP config adapter rather than defining a second resolution convention.
- XII: session generation claims and per-job refresh claims establish ownership
  before network/process awaits; no global coordination lock crosses an await.
- XVII: atomic snapshot replacement precedes live process reload, and only
  process status confirmation can produce refreshed/partially-refreshed.
- XVIII: persisted workspace/job affinity routes the request and the worker
  authorizes the exact execution ID.
- XIII: only the MCP section of a vendor config is edited atomically; unrelated
  content and runtime assets remain intact.
- XXI: errors name materialization versus Codex reload/bootstrap while rendering
  no MCP values or secrets.

No constitution deviations are planned.

## Risks & Dependencies

- The task depends on VAS-356's scoped-home implementation and therefore cannot
  be safely implemented on the stale branch base.
- Codex acknowledges reload before next-turn adoption; the existing pending
  generation and subsequent status enumeration must remain the success boundary.
- Worker recovery cannot recreate a live app-server control after process loss;
  such jobs must return unsupported/bootstrap failure rather than fabricate
  adoption.
- Process-lifetime ownership of the scoped home must remain unchanged while its
  refresh metadata is made reachable from `WorkerJob`.
