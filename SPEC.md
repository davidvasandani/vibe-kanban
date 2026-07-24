# Technical Specification: Shared Human/Agent Control for Workspace Browser Sessions

Task: `vk/57e0-add-shared-human`

## Summary

Add first-class, workspace-scoped **managed browser sessions** to Vibe Kanban,
surfaced as a new **Browser** right-main mode alongside Preview, Changes, and
Logs. A managed browser session is a server-owned Chromium instance that both
agents (via MCP tools) and humans (via the VK web UI) can observe concurrently,
but that exactly **one controller** may mutate at a time. Control is an
explicit, renewable lease with a monotonically increasing generation; humans
can take control from an agent (e.g. to complete an interactive login) and
return it to a specific agent execution without restarting the browser or
losing page state.

## Current state (verified against the codebase)

- **No browser automation exists anywhere in the repo.** All "browser" hits
  are: opening the user's desktop browser (`crates/utils/src/browser.rs`),
  relay auth sessions (`relay_browser_sessions` in `crates/relay-tunnel`), and
  the iframe dev-server preview proxy (`crates/preview-proxy`). This feature is
  greenfield.
- **Preview** is an iframe reverse proxy for the workspace's dev server
  (`crates/preview-proxy`, `packages/ui/src/components/PreviewBrowser.tsx`,
  `packages/web-core/src/pages/workspaces/PreviewBrowserContainer.tsx`). Per
  the task's non-goals it must remain untouched; Browser is a sibling mode.
- **Right-main modes** are a per-workspace zustand union
  (`packages/web-core/src/shared/stores/useUiPreferencesStore.ts:6-13`,
  `RIGHT_MAIN_PANEL_MODES = { CHANGES, LOGS, PREVIEW }`), rendered by
  `WorkspacesLayout.tsx` (desktop switch at ~476-500) with a per-mode upper
  section in `RightSidebar.tsx` (~141-190) and toggle actions in
  `shared/actions/index.ts` (~635-706).
- **Backend patterns**: services live in `crates/services`, are owned by
  `LocalDeployment` and exposed through the `Deployment` trait
  (`crates/deployment/src/lib.rs`); routes in `crates/server/src/routes`
  return `ApiResponse<T>`/`ApiError`; WebSockets use `SignedWsUpgrade` →
  `MaybeSignedWebSocket` (`crates/server/src/middleware/signed_ws.rs`) so
  relay-signed transport works transparently; realtime data uses JSON-patch
  streams over WS (`useJsonPatchWsStream.ts` on the frontend).
- **MCP** (`crates/mcp`) is an rmcp stdio server whose tools call the local VK
  HTTP API (`/api/...`) — the API is already the security boundary. Workspace
  scope is derived from the launch cwd via
  `/api/containers/attempt-context` and **cannot be overridden by tool args**
  in orchestrator mode (`scope_allows_workspace`). There is no ambient
  "calling execution id"; see §8.3 for how agent identity is resolved.
- **Identity**: the local deployment is single-user (`deployment.user_id()`),
  Workspace → Session → ExecutionProcess is the execution hierarchy
  (`crates/db/src/models/execution_process.rs`). "Agent execution" in this
  spec means an `ExecutionProcess` id.
- The design reference named in the task
  (`homelab/specs/vk/c80c-vk-preview-panel/research.md`) does not exist in
  this workspace; this spec is grounded in the codebase directly.
- No Chromium binary is present in the dev environment, so the real driver
  must be optional at runtime and all control semantics must be testable
  without it (§6, §12).

## 1. Goals

1. Workspace-scoped managed browser sessions with mandatory `workspace_id`
   and `host_id`, independent of Preview.
2. A single authoritative, host-affine **browser command gateway** through
   which every mutating path flows: REST actions, live-view WS input, and MCP
   tools.
3. A per-session **control arbiter**: `uncontrolled` /
   `agent-controlled(execution_id)` / `human-controlled(user_id,
   connection_id)`, implemented as a renewable lease with a monotonically
   increasing generation and compare-and-swap transfers.
4. Concurrent read-only observation (screencast, status, screenshots, page
   content, console) regardless of who controls the session.
5. Clean transfer semantics agent→human and human→agent(execution), with
   typed, retryable `CONTROL_LOST` for displaced agents and no replay of
   stale-generation commands.
6. Browser UI: Browser right-main mode, session sidebar, live view, controller
   toolbar with Take control / Return to agent.
7. Audit: controller transitions recorded (persisted) with reason,
   generation, and timestamp; live state stays authoritative in memory on the
   owning host.

## 2. Non-goals (from the task, binding)

- No merging of Preview iframe lifecycle with managed browser lifecycle.
- No raw-CDP port pools, Nix-pinned sessions, nftables, or infra notifications
  in VK core.
- No routing of high-frequency screencast frames or raw mouse movement through
  MCP.
- Multi-user RBAC beyond the existing single-user local deployment is out of
  scope; the design leaves seams (user_id on leases, `force` takeover flag)
  for multi-user deployments.

## 3. Architecture overview

```
                      ┌────────────────────────────────────────────┐
                      │  BrowserSessionService (crates/services)   │
                      │  ┌──────────────┐  ┌────────────────────┐  │
 REST /api/browser-…──┼─▶│ Command      │─▶│ per-session actor  │  │
 WS live-view input ──┼─▶│ Gateway      │  │  - ControlArbiter  │  │
 MCP tools ──(HTTP)───┼─▶│ (ownership + │  │  - serialized cmd  │  │
                      │  │  generation  │  │    queue           │  │
                      │  │  checks)     │  │  - BrowserDriver   │  │
                      │  └──────────────┘  └─────────┬──────────┘  │
                      │        ▲                     │             │
                      │   read-only paths            ▼             │
 WS observers ◀───────┼── state broadcast      Chromium (CDP)      │
                      └────────────────────────────────────────────┘
```

- MCP tools call the REST API (existing pattern), so REST is the single
  enforcement surface for MCP; the live-view WS shares the same service-level
  gateway. Nothing mutates the browser without passing
  `BrowserSessionService::execute_command`.
- One **session actor** (tokio task) per browser session serializes all
  mutating commands; the arbiter state lives inside the service (in-memory,
  host-authoritative). The DB stores session rows and an audit trail of
  control transitions, never the live lock.

## 4. Data model

### 4.1 New table `browser_sessions` (SQLite migration)

| column            | type | notes                                             |
|-------------------|------|---------------------------------------------------|
| id                | TEXT PK (UUID) |                                         |
| workspace_id      | TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE |
| host_id           | TEXT NOT NULL | local deployment: the local host/server id |
| profile           | TEXT NULL | named profile (user-data-dir key), redacted in events |
| status            | TEXT NOT NULL | `starting` \| `running` \| `closed` \| `failed` |
| current_url       | TEXT NULL | last known URL (redacted in activity events)  |
| created_at / updated_at / closed_at | TEXT | `datetime('now','subsec')` pattern |
| expires_at        | TEXT NULL | idle-expiry policy deadline                   |

### 4.2 New table `browser_control_transitions` (audit)

`id, browser_session_id (FK CASCADE), generation INTEGER NOT NULL, controller
kind/holder columns (controller_type TEXT: none|agent|human; execution_id TEXT
NULL; user_id TEXT NULL; connection_id TEXT NULL), reason TEXT NOT NULL
(acquire|release|transfer|takeover|expired|disconnected|execution_completed|
closed), created_at`. Append-only; written on every transition, after the
in-memory transition commits (persist-for-audit, not lock).

### 4.3 In-memory control state (authoritative)

```rust
enum Controller {
    None,
    Agent { execution_id: Uuid },
    Human { user_id: String, connection_id: Uuid },
}
struct ControlState {
    controller: Controller,
    generation: u64,          // strictly monotonic per session
    lease_expires_at: Instant, // renewable; ignored for Controller::None
}
```

Lease TTL: 60s, renewed implicitly by any accepted command from the holder
and explicitly via acquire/renew. Human leases are additionally bound to the
live-view WS `connection_id`: the socket closing releases the lease.

## 5. Control arbiter semantics

All transitions happen atomically inside the session actor (single-threaded
per session), so CAS is a plain compare in that context.

- **Acquire** (`expected_generation` optional): allowed iff controller is
  `None` or the lease has expired, else `409 CONTROL_CONFLICT` describing the
  current controller + generation. On success: `generation += 1`, lease set.
- **Release**: only the current holder (or a privileged `force`) may release.
  `generation += 1`, controller → `None`.
- **Transfer** (`expected_generation` required — CAS): current holder (human
  in the UI flow) hands control to a named target
  (`agent(execution_id)` or `none`). Mismatched generation → `409
  STALE_GENERATION`. On success `generation += 1`.
- **Take control (human)**: a human acquire with `take_from_agent: true`
  displaces an agent lease at a command boundary (§5.1). Another **agent** can
  never displace a live controller — agents only acquire `None`/expired
  sessions. A privileged `force: true` (UI-only affordance today) can displace
  a human lease (multi-user seam).
- **Every mutating command** carries the resolved controller identity and the
  generation it believes is current; the actor rejects commands whose
  generation ≠ current generation with typed `CONTROL_LOST` (agent path) /
  `STALE_GENERATION` (API path) **without executing or queueing them**.

### 5.1 Agent → human takeover sequence (in-actor)

1. Mark session `transferring`: gateway stops admitting new mutations for the
   old generation.
2. The currently executing atomic command (single CDP command or bounded
   navigation wait) finishes or is cancelled at its timeout boundary.
3. All queued commands from the old generation are drained and completed with
   `CONTROL_LOST` — never executed later.
4. `generation += 1`; controller → `Human { user_id, connection_id }`.
5. Broadcast new controller state to all observers; append audit row.
6. Any subsequent MCP mutation from the old execution gets typed
   `CONTROL_LOST { current_controller, generation }` (retryable: the agent may
   re-acquire later if control is released back).

### 5.2 Human → agent return

1. Human (holder) calls transfer with `expected_generation` and target
   `agent(execution_id)` (chosen in the UI from the workspace's recent/running
   executions) or `none`.
2. `generation += 1`; broadcast; audit. The agent resumes on the same
   Chromium instance — cookies, storage, and page state intact.

### 5.3 Automatic release

- **Lease expiry**: holder inactivity past TTL → controller `None`,
  `generation += 1`, reason `expired`.
- **Human WS disconnect**: leases bound to `connection_id` release on socket
  close, reason `disconnected`.
- **Execution completion**: when an `ExecutionProcess` finishes, its leases
  release (reason `execution_completed`) but sessions stay alive until closed
  or expired by the session-idle policy.
- **Session close/workspace archive**: closing a session kills Chromium and
  marks the row `closed`; archiving/deleting a workspace closes its sessions
  (cleanup policy hook).

## 6. Browser driver

```rust
#[async_trait]
trait BrowserDriver: Send + Sync {
    async fn launch(&self, opts: LaunchOpts) -> Result<DriverHandle, DriverError>;
}
// DriverHandle: navigate, click, type_text, key, set_viewport, evaluate,
// screenshot, start/stop screencast (frame stream), page_info, close —
// each with bounded timeouts.
```

- **CdpDriver**: spawns a locally installed Chromium
  (`VIBE_BROWSER_CHROMIUM_PATH` or discovered from a standard candidate list)
  with `--remote-debugging-port=0`, headless, per-profile `--user-data-dir`
  under the VK data dir, and speaks raw CDP over
  `tokio-tungstenite` (already in the workspace tree — no heavy new
  dependency): `Target.*`, `Page.navigate`, `Page.startScreencast`,
  `Input.dispatchMouseEvent/dispatchKeyEvent`, `Emulation.setDeviceMetricsOverride`,
  `Runtime.evaluate`, `Page.captureScreenshot`.
- **MockDriver** (`#[cfg(test)]` + used when no Chromium found in dev):
  records commands, emits synthetic frames/URLs. All arbiter/gateway/API tests
  run against it. If no Chromium binary is available, session creation fails
  with typed `BROWSER_UNAVAILABLE` (the UI shows this state); everything else
  remains functional.
- `Runtime.evaluate` is a **privileged capability**: gated behind a
  per-request `capability: "evaluate"` check (config flag
  `browser.allow_evaluate`, default true locally — seam for multi-user).

## 7. REST API (crates/server, new `routes/browser_sessions.rs`)

All responses `ApiResponse<T>`; errors via `ApiError` with a new
`BrowserSessionError` variant carrying typed codes:
`CONTROL_CONFLICT`, `STALE_GENERATION`, `CONTROL_LOST`, `SESSION_CLOSED`,
`BROWSER_UNAVAILABLE`, `CAPABILITY_DENIED`.

```
POST   /api/browser-sessions                     { workspace_id, profile? }
GET    /api/browser-sessions?workspace_id=…      list (live status merged)
GET    /api/browser-sessions/{id}                detail incl. control state
DELETE /api/browser-sessions/{id}                close (requires control or force)

GET    /api/browser-sessions/{id}/control        { controller, generation, lease_expires_at }
POST   /api/browser-sessions/{id}/control/acquire   { as: agent|human, execution_id?, take_from_agent?, force?, expected_generation? }
POST   /api/browser-sessions/{id}/control/release   { expected_generation }
POST   /api/browser-sessions/{id}/control/transfer  { expected_generation, target: none|{execution_id} }

POST   /api/browser-sessions/{id}/actions        { command_id, expected_generation, action }
       action = navigate|back|forward|reload|click|type|key|set_viewport|evaluate
GET    /api/browser-sessions/{id}/screenshot     read-only, concurrent
GET    /api/browser-sessions/{id}/page           read-only url/title/console tail
```

- `command_id` (client UUID) provides idempotency: the actor keeps a small
  LRU of executed command ids per generation and returns the recorded result
  on duplicates.
- Human REST/WS callers resolve identity as `deployment.user_id()`; agent
  callers must name an `execution_id` that belongs to the session's workspace
  (validated server-side; see §8.3).
- Workspace authorization: session routes load the session row and its
  workspace via loader middleware (existing `model_loaders.rs` pattern);
  MCP-scoped calls additionally pass the MCP scope check client-side as today.

### 7.1 Live-view WebSocket

`GET /api/browser-sessions/{id}/ws` via `SignedWsUpgrade`.

Server → client (JSON, plus binary frames):
- `{"type":"ready", state: BrowserSessionLiveState}` on connect
- `{"type":"state", …}` on any state/controller change (broadcast)
- `{"type":"frame", meta}` + binary JPEG payload (screencast; observers all
  receive frames — read-only, never gated on control)
- `{"type":"command_result", command_id, result|error}`

Client → server:
- `{"type":"input", command_id, expected_generation, action}` — mouse/key/nav
  input; **enforced through the same gateway** (must hold a human lease bound
  to this connection).
- `{"type":"acquire"|"release"|"transfer", …}` — control ops (equivalent to
  REST; bound to this `connection_id`).

The WS assigns a server-side `connection_id` (UUID) at upgrade; human leases
record it; the socket's close handler releases matching leases.

### 7.2 Session-list state stream

Sidebar needs live session lists per workspace. Sessions rows are DB-backed:
register `browser_sessions` in the events hook tables
(`services/src/services/events.rs`) and add a filtered JSON-patch stream +
`GET /api/workspaces/{id}/browser-sessions/ws` … **or** reuse the per-session
WS `state` messages plus REST list refresh. Decision: DB-hook stream for the
list (status/url rows), per-session WS for controller/generation (live-only
data). Controller state is *also* mirrored into list payloads on transition
via a `msg_store` patch emitted by the service (not read from DB).

## 8. MCP tools (crates/mcp)

New tool module `browser.rs`, registered in both global and orchestrator
routers. All tools call the REST API above. Low-frequency only (no screencast
frames, no raw mouse-move).

| tool | behavior |
|------|----------|
| `browser_create_session` | create (or return existing running) session for the scoped workspace |
| `browser_list_sessions` | list sessions for scoped workspace |
| `browser_get_control` | control state incl. generation |
| `browser_acquire_control` | acquire for the calling execution (§8.3); never displaces a live controller |
| `browser_release_control` | release own lease |
| `browser_navigate` / `browser_click` / `browser_type` / `browser_key` / `browser_evaluate` | mutating actions; auto-acquire an uncontrolled session for the calling execution, else typed `CONTROL_LOST`/`CONTROL_CONFLICT` in the error payload |
| `browser_screenshot` / `browser_get_page` | read-only, always allowed |

- Every tool result includes `{ workspace_id, browser_session_id, controller,
  generation }` so agents can reason about state.
- `CONTROL_LOST` surfaces in the standard in-band error JSON
  (`{"success":false,"error":"CONTROL_LOST",…, "retryable": true}`) so agents
  pause browser work cleanly instead of crashing.

### 8.3 Agent identity resolution

MCP tools have no ambient execution id (verified: `crates/mcp` exposes only
workspace scope + optional orchestrator session id). Resolution, server-side:

1. Tool sends `workspace_id` (scope-resolved; args cannot widen scope).
2. The acquire/action endpoint receives `as: "agent"` without explicit
   `execution_id`; the server resolves **the workspace's currently running
   coding-agent `ExecutionProcess`** (most recently started `Running` process
   with `run_reason = CodingAgent` across the workspace's sessions).
3. If none is running, the acquire is rejected (`NO_RUNNING_EXECUTION`); an
   explicit `execution_id` argument is accepted only if that execution belongs
   to the scoped workspace.

This keeps execution ownership meaningful (leases die with the execution)
without letting tools claim arbitrary executions in other workspaces.

## 9. Frontend (packages/web-core, ui)

1. **Mode**: add `BROWSER: 'browser'` to `RIGHT_MAIN_PANEL_MODES` +
   `RightMainPanelMode`; scratch persistence mapping; `ToggleBrowserMode`
   action (`V B`) in `shared/actions/index.ts`; render branch in
   `WorkspacesLayout.tsx` (desktop + mobile) → `BrowserPanelContainer`.
2. **Right-main surface** `BrowserPanelContainer.tsx`: live view canvas/img
   fed by screencast frames over the session WS; toolbar showing URL, status,
   and **controller state**: "You are controlling", "Agent controlling ·
   execution 456", or "No controller", with `Take control` / `Return to
   agent` / `Release` buttons (Return-to-agent opens a picker of the
   workspace's recent executions, defaulting to the one control was taken
   from). Read-only observers keep receiving frames during transfers.
   When the human holds control, pointer/keyboard events on the live view are
   translated to `input` WS messages (client-side coalescing for mouse-move;
   raw movement stays on the WS path, never MCP).
3. **Sidebar** upper section for `BROWSER` in `RightSidebar.tsx` →
   `BrowserControlsContainer`: session list (live), New session (+ profile
   name), per-session status/expiry/current controller/generation, close.
4. **API client**: `browserSessionsApi` in `shared/lib/api.ts`; WS via
   `openLocalApiWebSocket` so remote-web transport swapping keeps working.
5. **Types** from `shared/types.ts` (ts-rs): `BrowserSession`,
   `BrowserSessionControlState`, `BrowserController`, `BrowserAction`,
   `BrowserSessionLiveState`, error codes enum, WS message unions.

## 10. Events / audit / redaction

- Controller transitions append `browser_control_transitions` rows and emit a
  VK event-stream patch (activity history). Events carry controller kind,
  execution id, generation, reason, timestamp — **not** URLs or profile
  contents (`current_url` redacted to origin, profile to its name hash) per
  the task's redaction requirement.
- Command-level logging stays at debug level, no page content in logs.

## 11. Config / deployment wiring

- `BrowserSessionService` constructed in `LocalDeployment::new`, exposed via
  new `Deployment::browser_sessions()` trait method.
- Config additions (existing config version pattern): `browser.enabled`
  (default true), `browser.chromium_path?`, `browser.allow_evaluate`,
  `browser.session_idle_expiry_minutes` (default 120), `browser.lease_ttl_secs`
  (default 60).
- `host_id`: local deployment uses the existing local host identity (same id
  used by preview/relay host scoping); sessions created on this host are
  served only by it (host-affinity enforced by construction locally; the field
  future-proofs remote/multi-host).

## 12. Testing

Rust (`cargo test --workspace`), all against MockDriver:
1. Arbiter unit tests: acquire on uncontrolled; competing concurrent acquires
   (exactly one wins); CAS transfer with stale generation rejected; lease
   expiry; renew-on-command.
2. Takeover: queued old-generation commands drained as `CONTROL_LOST`, never
   executed after transfer; in-flight command cancelled at boundary.
3. Agent→human→agent round-trip preserving driver state (mock asserts no
   relaunch); returned execution can act under new generation.
4. Disconnect cleanup (connection-bound lease released on WS close) and
   execution-completion cleanup (lease released, session alive).
5. Idempotency: duplicate `command_id` returns recorded result, no re-execute.
6. Route tests for typed error codes; MCP tool tests for auto-acquire and
   in-band `CONTROL_LOST`.

Frontend: `pnpm run check` + `pnpm run lint`; Vitest for the controller-state
reducer/format helpers if runtime logic is added.

## 13. Acceptance criteria

The task's acceptance list is adopted verbatim (Browser mode separate from
Preview; live agent actions visible; takeover without state loss; typed,
non-replayed CONTROL_LOST; return-to-execution; atomic generation CAS; stable
observers; predictable release on disconnect/completion/expiry; one backend
arbiter for REST+MCP+WS; immediate toolbar/sidebar reflection + activity
history; the test matrix of §12).

## 14. Risks / open questions

- **No Chromium in CI/dev sandbox** → real-driver path is exercised manually;
  gate e2e behind binary discovery (typed `BROWSER_UNAVAILABLE` otherwise).
- **Screencast bandwidth**: CDP `Page.screencast` JPEG frames are throttled
  (`everyNthFrame`, quality knobs); observers share one screencast per session
  fanned out via broadcast channel.
- **Execution-id ambiguity** with multiple concurrent running executions in a
  workspace (§8.3) — resolved by most-recent-running + explicit override
  within scope.
- **Remote deployment** (crates/remote) is out of scope; the API shape
  (host_id, redacted events) is designed not to preclude it.
