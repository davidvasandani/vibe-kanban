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
control semantics are tested against the mock (CI has no Chromium).
The screencast ack must be sent fire-and-forget from the CDP reader task —
awaiting a response there deadlocks the reader on itself.

### Real Chromium in the dev/test environment

The VK dev host now has a real Chromium for exercising the CDP path. On this
NixOS host, FHS chrome/headless-shell downloads (Playwright's, Google's) do
not run — use a Nix-built chromium. It was provisioned from an
already-realized store path (no download):

```
nix-store --add-root ~/.local/share/vibe-kanban/chromium-gc-root \
  -r /nix/store/<hash>-chromium-<ver>       # pin against GC
ln -sf /nix/store/<hash>-chromium-<ver>/bin/chromium \
  ~/.local/share/vibe-kanban/cli-tools/bin/chromium   # already on VK's PATH
```

`discover_chromium` finds it via PATH, so the server uses the real driver by
default; `VK_BROWSER_MOCK=1` still forces the mock. Verified live against
the real driver: session create spawns Chromium (~11 procs), navigate,
`Input.insertText` into an autofocused field read back via `evaluate`,
`Page.captureScreenshot` returns a real PNG, screencast delivers real JPEG
frames (SOI-checked) over the live-view WS, and close reaps the entire
process group (D-Bus errors on stderr are benign headless noise). Follow-up
seam: make the install declarative in the homelab host module instead of
this imperative GC-root + symlink.

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

## Cloudflare Access headers (origin-scoped)

`access_headers.rs` lets managed browsers present a Cloudflare Access service
token, so agents can open Access-gated apps without an email login. It is
active only when `VIBE_BROWSER_CF_ACCESS_CLIENT_ID`, `_CLIENT_SECRET` and
`_DOMAINS` are all set. If there are no domains, nothing is sent (fail closed).

- Use `Fetch.enable` with URL patterns, not `Network.setExtraHTTPHeaders`.
  The latter adds headers to every request, third parties included.
- CDP `urlPattern` wildcards span any characters. A pattern like
  `https://*.example.com/*` misses explicit ports (`:8443`) and trailing-dot
  hosts, and it also matches `https://evil.net/?.example.com/`. So the driver
  pauses `https://*example.com*` and decides per request with `applies_to`:
  https only, no IP hosts, and an exact or label-boundary subdomain match. The
  test `fetch_patterns_pause_every_in_scope_url` emulates CDP glob matching
  to prove every in-scope URL form pauses.
- Every paused request must get `Fetch.continueRequest` or it hangs. The
  reader answers with `send_no_wait`, the same rule as screencast acks.
- Header overrides do not carry across redirects. Each hop that matches a
  pattern pauses again and gets the headers again.
- Interception covers the attached page target only. Popups and new tabs are
  not covered.
- Live check (ignored by default):
  `VK_BROWSER_LIVE_URL=https://<gated host>/ cargo test -p services --lib live_access_headers -- --ignored`.

## Mobile keyboard and plain-text clipboard (vk/5c0e-vk-browser)

A screencast image has no local editable DOM target. Focusing its wrapper
cannot summon a software keyboard. Keep a real labeled textarea mounted and
focus it synchronously during the touch gesture, before awaiting remote input.
VK focuses it on touch taps inside the rendered frame and exposes Keyboard as
an explicit alternative; taps on non-editable remote content can therefore
also open the keyboard. Capture frame coordinates before focus changes the
mobile viewport. A sentinel behind the local caret lets soft keyboards emit
Backspace after each committed value has been cleared. Ignore intermediate
composition events, commit once, and leave printable keys to native input in
the textarea so keydown and input do not duplicate text.

Clipboard operations use the existing WebSocket action acknowledgement and
control gate. `copy_selection` evaluates a fixed expression, without enabling
arbitrary Evaluate, and returns the selected plain text from a focused input,
textarea, same-origin frame/open shadow root, or document selection. Password
inputs are excluded. Copy fails above 65,536 UTF-16 code units instead of
silently truncating; the UI applies the same bound to inserted text. Cross-origin
frames are not readable through this expression. Paste uses Type / CDP
`Input.insertText`, which replaces the remote selection but does not synthesize
a remote paste event.

Preserve the browser's local Ctrl/Cmd+V default and consume `clipboardData` in
its paste event. A Paste button can call `navigator.clipboard.readText()`.
Async clipboard permission can be denied, and Copy's remote response can
outlive Safari's user activation, so offer a manual textarea fallback. Remount
the bridge on session/connection/control-generation changes; unmount it on
control loss/disconnection and reject late async results before clipboard or
remote side effects. Do not persist or log text.

Chromium needs virtual key codes for Select all, deletion and navigation.
Avoid text payloads on Ctrl/Meta shortcuts, but preserve AltGr/Option-produced
characters: the client OS already resolved them, so insert that text directly
rather than treating Ctrl+Alt as a remote shortcut. Keep punctuation out of the
ASCII virtual-key shortcut mapping (`.` is ASCII 46, the Delete virtual key).
Use the ignored `live_keyboard_and_selection` Rust test with local Chromium to
verify actual editing behavior. DOM/touch-event tests prove synchronous focus
and forwarding, not whether a physical phone displays its OS keyboard.

## Contributed by

- vk/57e0-add-shared-human
- vk/5673-cloudflare-acces
- vk/5c0e-vk-browser (workspace `5c0e77f9-1063-4612-9ac7-922daf57452b`)

## Touch and wheel scrolling (vk/5c0e-browser-scrolling)

The remote browser needs an explicit `BrowserAction::Scroll` with integer
`x`, `y`, `delta_x`, and `delta_y`. CDP receives `Input.dispatchMouseEvent`
with `type: mouseWheel`, so hit testing targets nested scroll containers.
It uses the same ownership/generation gate as other input. Regenerate shared
types after extending the Rust action enum.

The live surface attaches a non-passive wheel listener. Normalize pixel, line
(16 CSS pixels), and page deltas, then scale to the remote viewport. Keep
fractional deltas until integer dispatch and retain the remainder; rounding
each high-resolution trackpad event can discard sustained scrolling. Bound
work to one in-flight command and one accumulated delta. Pending movement is
tied to its hit point; when that point changes, discard pending movement
rather than applying another scroll container's movement to the new target.

Touch/pen pointer capture distinguishes taps from swipes with an 8 CSS-pixel
threshold. Swipes reverse finger displacement into wheel deltas at the
original hit point. They never click or focus the keyboard. Cancel gestures
on additional pointers, pointer cancellation, capture loss, disconnection,
session change or control-generation change. Use `touch-action: none` only
while controlling the remote surface. Keep gesture dependencies tied to frame
width/height, not the per-frame metadata object: every incoming video frame
otherwise resets an active gesture.

Regression coverage lives in `BrowserPanelContainer.test.tsx` and the Rust
browser tests. `live_scroll_targets_nested_content` is opt-in and verifies
actual two-axis nested scrolling with local Chromium. If discovery fails,
set `VIBE_BROWSER_CHROMIUM_PATH` to the installed Nix Chromium binary. These
checks do not replace a physical phone test.

Task: `vk/5c0e-browser-scrolling`, workspace
`5c0e77f9-1063-4612-9ac7-922daf57452b`.
