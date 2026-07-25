# Browser Sessions: One Arbiter, Lease + Generation Control, and the Driver Seam

How workspace-scoped managed browser sessions
(`crates/services/src/services/browser/`) implement shared human/agent
control, and the reusable patterns/gotchas from building them. Complements
[agent-process-lifecycle.md] (execution identity, pgid kill discipline) and
Constitution P9 (one arbiter per shared mutable resource).

## The concurrency shape that survived adversarial review

One `SessionRuntime` per live browser session, with **three separate locks**
whose roles must not be merged:

- `control: std::sync::Mutex<ControlArbiter>` — the pure lease/generation
  state machine. Never held across an await; every check-then-act is atomic
  under it, so compare-and-swap is a plain compare.
- `command_gate: tokio::sync::Mutex<()>` — serializes **mutating** commands
  only. A command records its admitted generation *before* waiting for the
  gate and re-checks it *after* acquiring it: a takeover that lands in
  between turns the queued command into a typed `CONTROL_LOST` without
  touching the browser ("invalidate, never replay" with no queue-drain
  machinery at all).
- `handle: std::sync::RwLock<Option<Arc<dyn DriverHandle>>>` — read-only
  operations (screenshot, page info) clone the `Arc` synchronously and never
  take the command gate, so observers stay live while a controller runs a
  long navigation. First version put reads behind the same tokio mutex as
  writes; Codex flagged it as contradicting the "observers stay concurrent"
  invariant.

Control ownership is a renewable lease (`lease_expires_at`) with a strictly
monotonic `generation`. Every transition (+1) invalidates all commands
admitted under older generations. Humans take over agents explicitly
(`take_from_agent`); agents can *never* displace a live controller; `force`
is a human-only seam for multi-user deployments. Closing a session requires
holder / uncontrolled / force — a pure `close_permitted` function extracted
specifically so it is unit-testable (the buggy human-over-agent special case
survived one review round because the logic was inline).

## Idempotency needs in-flight reservation, not just a result cache

A completed-results cache alone is racy: two concurrent requests with the
same `command_id` both miss and both execute. The fix is atomic
check-and-reserve: the first caller inserts `InFlight(watch::Receiver)` and
becomes owner; concurrent duplicates await the watch; the owner publishes the
outcome and converts the entry to `Done`. Two subtleties:

- **Owner cancellation** (axum drops request futures on client disconnect): a
  drop-guard removes the still-`InFlight` entry so the command id is not
  poisoned; waiters map the closed channel to a retryable timeout.
- **Cache typed errors, not strings.** Storing `err.to_string()` made a
  replayed `TIMEOUT` come back as `DRIVER_ERROR`, losing retryability and
  status mapping. Store the typed error enum (it derives `Clone`).
- Only commands that *reached the driver* are recorded durably (success,
  driver error, ambiguous timeout). Control rejections are not executions —
  a retry under a fresh generation must be allowed to run.

## serde_json `preserve_order` breaks f64 in internally-tagged enums

This workspace enables serde_json's `preserve_order`. Deserializing an
internally-tagged enum (`#[serde(tag = "type")]`) whose variant has an `f64`
field fails with `invalid type: map, expected f64` — even in a minimal repro.
Integer and string fields are fine. Consequence: wire enums like
`BrowserAction` use `i32` coordinates (clients round). Check this before
designing any new tagged wire enum with float fields.

## Typed errors through the single-message ApiError channel

`ApiError` responses carry only a message string. The browser routes
serialize the typed `BrowserSessionError` (serde-tagged, stable `code` field,
controller + generation context) *as* that message; MCP tools and the
frontend parse it back. This keeps one machine-readable contract across
REST, WS, and MCP without changing the global error envelope. MCP surfaces it
in-band as `{success:false, error: CODE, retryable: bool, details: {...}}` so
agents pause on `CONTROL_LOST` instead of crashing.

## Event-driven cleanup instead of container hooks

Execution-completion and workspace-archival cleanup subscribe to the existing
SQLite-hook JSON-patch stream (`EventService` msg store) rather than
threading the browser service into `LocalContainerService` finalization:
execution patches leaving `running` release that execution's leases;
workspace patches with `archived: true` (or removes) close its sessions.
Broadcast lag can drop events — the lease TTL and idle sweep are the
backstop, which is what makes this design safe. The sweeper holds only a
`Weak` to the service between ticks (a strong clone in the loop leaks the
service forever — same class of bug as the keep-warm poll loop).

## Driver seam and degraded environments

`BrowserDriver`/`DriverHandle` traits isolate Chromium: `CdpDriver` speaks
raw CDP over tokio-tungstenite 0.26 (repo standard; ~10 methods, no
chromiumoxide dependency) to a Chromium spawned with
`--remote-debugging-port=0` in its own process group and killed via
`utils::process::kill_process_group` (never `kill_on_drop` — it misses
Chromium's helpers). No binary discovered → `UnavailableDriver` → typed
`BROWSER_UNAVAILABLE`; `VK_BROWSER_MOCK=1` opts into the mock driver. All
control semantics are tested against the mock (no Chromium exists in CI).
The screencast ack must be sent fire-and-forget from the CDP reader task —
awaiting a response there deadlocks the reader on itself.

## Trust model (recorded seam)

The local deployment is single-user and MCP reaches the API as plain local
HTTP, so `as: "agent"` binds to a running execution in the session's
workspace on the caller's word; workspace membership is the enforced
boundary. Per-execution caller authentication (threading execution identity
into MCP launch env) is the documented hardening seam for multi-user
deployments — see `homelab/specs/vk/57e0-add-shared-human/research.md`.

## Headless runtime verification (repeatable)

The full control lifecycle was verified against the real server binary, not
just unit tests. The recipe generalizes to any headless VK smoke test:

- Build and run `./target/debug/server` with `VK_BROWSER_MOCK=1`,
  `BACKEND_PORT=<free>`, **and `PREVIEW_PROXY_PORT=0`** — task worktrees
  inherit the host VK's `PORT`/`PREVIEW_PROXY_PORT` env vars, so an
  unoverridden proxy port fails the boot with a bare `AddrInUse` that names
  no listener. Dev builds use `<repo>/dev_assets/` (gitignored) for the DB,
  so the run is isolated; delete `dev_assets/` afterwards.
- Seed FK prerequisites directly with `sqlite3`: UUID keys are 16-byte
  blobs (`X'<uuid-hex-no-dashes>'`), e.g. a `workspaces` row, and a
  `sessions` + `execution_processes(status='running',
  run_reason='codingagent')` pair to stand in for a live agent execution.
- Drive REST with curl; drive the live-view WS with Node ≥22's **built-in
  WebSocket** (no deps): `binaryType='arraybuffer'`, JSON `frame` meta is
  followed by one binary message.
- Verified end-to-end this way: create → human acquire (gen 1) → navigate →
  duplicate command_id replay (no re-exec) → NO_RUNNING_EXECUTION →
  STALE_GENERATION on CAS → transfer to seeded execution (gen 2) → agent
  auto-resolved action → takeover (gen 3) → stale agent mutation 409 →
  release → WS acquire bound to connection (gen 5) → WS input + frame
  broadcast → disconnect auto-release (gen 6, reason `disconnected`) →
  audit rows with no URL/profile → plain close of uncontrolled session.

## Contributed by

- vk/57e0-add-shared-human
