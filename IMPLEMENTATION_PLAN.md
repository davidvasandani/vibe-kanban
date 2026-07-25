# Implementation Plan: Shared Human/Agent Control for Workspace Browser Sessions

Task: `vk/57e0-add-shared-human`. Builds on `SPEC.md` and
`PRIOR_KNOWLEDGE.md`. Ordered so each layer is compilable/testable before the
next; the control arbiter is implemented and tested before any real browser
plumbing.

## Phase 1 — Core domain (crates/services)

1. `services/src/services/browser/` module skeleton:
   `types.rs` (Controller, ControlState, BrowserCommand, typed error codes),
   `arbiter.rs` (pure lease/generation state machine: acquire / release /
   transfer-CAS / take_from_agent / expiry; unit-testable without tokio).
2. `driver.rs`: `BrowserDriver` + `DriverHandle` traits with bounded-timeout
   method signatures; `MockDriver` recording commands and emitting synthetic
   frames/URLs.
3. `session_actor.rs`: per-session tokio task owning arbiter + driver handle +
   serialized command queue + `command_id` idempotency LRU; implements the
   takeover drain (old-generation queue → CONTROL_LOST, in-flight cancel at
   timeout boundary) and broadcast of `BrowserSessionLiveState` over a
   `tokio::sync::broadcast` channel.
4. `service.rs` (`BrowserSessionService`): session registry
   (insert-before-remove discipline), create/get/list/close, lease-expiry and
   idle-expiry sweeps (generation-conditional reap), execution-completed and
   connection-closed release hooks.
5. Unit tests (per SPEC §12 items 1–5): competing acquires, stale generation,
   takeover drain, agent→human→agent round trip with state preserved,
   disconnect/execution cleanup, idempotent command replay, lease expiry.

## Phase 2 — Persistence + events

6. Migration `browser_sessions` + `browser_control_transitions`; models in
   `db/src/models/browser_session.rs` (+ registration in `models/mod.rs`).
7. Service persists session rows and appends audit transitions after
   in-memory commit; wire `browser_sessions` into events hook tables
   (`events.rs`, `events/patches.rs`, `events/streams.rs`) for the sidebar
   list stream; service pushes controller-state patches into the msg store on
   transitions.
8. Execution-completion hook: on execution finalization, call
   `browser_sessions().release_for_execution(execution_id)` (wired where the
   container finalizes executions; keep it a narrow trait-exposed call).

## Phase 3 — Deployment + REST + WS

9. Wire `BrowserSessionService` into `LocalDeployment::new`, add
   `Deployment::browser_sessions()`; config additions (enabled,
   chromium_path, allow_evaluate, expiry knobs) in the current config version.
10. `server/src/routes/browser_sessions.rs`: CRUD + control + actions +
    screenshot/page routes per SPEC §7; loader middleware; typed
    `BrowserSessionError` → `ApiError` mapping (409 for
    CONTROL_CONFLICT/STALE_GENERATION/CONTROL_LOST, 503 BROWSER_UNAVAILABLE);
    register in `routes/mod.rs`.
11. Live-view WS endpoint (SignedWsUpgrade): ready/state/frame/command_result
    messages; input + control client messages routed through the same
    service gateway; connection_id assignment and disconnect release.
12. Route-level tests (axum) for control endpoints and typed errors against
    MockDriver.

## Phase 4 — Types + MCP

13. ts-rs derives on all API/WS types; add `::decl()` lines in
    `generate_types.rs`; run `pnpm run generate-types`.
14. `crates/mcp/src/task_server/tools/browser.rs`: session, control, action,
    and read-only tools per SPEC §8; auto-acquire semantics; in-band
    retryable CONTROL_LOST payloads; register router in both modes; tool
    tests.

## Phase 5 — CDP driver (real browser)

15. `cdp.rs`: Chromium discovery (config → env → candidate paths), spawn with
    `--remote-debugging-port=0` + per-profile user-data-dir + process-group
    kill on close (pgid discipline from PRIOR_KNOWLEDGE, not kill_on_drop);
    minimal CDP client over tokio-tungstenite (navigate, input dispatch,
    viewport, evaluate-gated, screenshot, screencast start/stop with frame
    ack); screencast fan-out through the session broadcast channel.
16. Fallback selection: real driver when a binary is found, otherwise
    `BROWSER_UNAVAILABLE` on create; MockDriver stays test-only except via
    explicit env override for dev (`VK_BROWSER_MOCK=1`).

## Phase 6 — Frontend

17. Store/mode: add `BROWSER` to `RIGHT_MAIN_PANEL_MODES`, scratch
    persistence mapping, `ToggleBrowserMode` (V B) action, layout render
    branches (desktop + mobile).
18. `browserSessionsApi` + `useBrowserSessionWs` hook (openLocalApiWebSocket;
    binary frame handling; state reducer with Vitest tests).
19. `BrowserPanelContainer` (live view + controller toolbar: You are
    controlling / Agent controlling · execution N / Take control / Return to
    agent picker / Release) and `BrowserControlsContainer` sidebar section
    (session list via list stream, launch/close/profile/status/expiry);
    register in `RightSidebar.tsx`.
20. Input capture on the live view (pointer/keyboard → WS input messages,
    mouse-move coalescing) only while holding control; read-only otherwise.

## Phase 7 — Verification & polish

21. `cargo test --workspace`, `pnpm run check`, `pnpm run lint`,
    `pnpm run generate-types:check`, `pnpm run prepare-db`, `pnpm run format`;
    fix fallout. Manual smoke of the UI against MockDriver via
    `VK_BROWSER_MOCK=1` if a dev server can run.
22. Activity/audit verification: transitions appear in the event stream with
    redacted URL/profile fields.
23. Stage 11 Codex review loop; stage 12 knowledge-base distillation
    (new page on the control-arbiter/lease pattern + browser driver seam).

## Explicit scope guards

- Preview code paths untouched (`preview-proxy`, PreviewBrowser*).
- No screencast/raw-mouse through MCP.
- Single-user auth model retained; user_id/force fields are seams only.
