# Technical Plan: OAuth login for managed CLI tools

**Feature dir**: `specs/003-cli-tool-oauth-login/`
**Task**: `vk/5a2a-vk-cli-tool-logi`
**Spec**: [`spec.md`](spec.md)

## Approach

Extend the existing curated CLI-tool catalog with declarative authentication
metadata and bounded probes. Generalize the existing `PtyService` so it can
spawn a specific executable as well as an interactive shell, then expose a
machine-aware WebSocket endpoint dedicated to a catalog-owned login command.
Render that session in the CLI Tools settings row with the existing xterm stack.

This keeps command selection server-owned, prevents arbitrary-command execution,
preserves host-before-app binary precedence, and leaves credentials entirely in
the vendor CLI's normal files/keychain.

## Grounding

- `crates/services/src/services/cli_tools.rs`: catalog, copy detection,
  install/update/remove locks and status API. Add auth metadata, effective-binary
  resolution, bounded probes, and per-tool login locks here.
- `crates/local-deployment/src/pty.rs`: existing `portable_pty` session lifecycle.
  Extract common spawn logic and add command sessions plus exit notification.
- `crates/server/src/routes/cli_tools.rs`: existing machine API. Add the login
  WebSocket route, validate catalog support, and bridge PTY input/output/resize.
- `crates/server/src/routes/terminal.rs`: reuse/extract its base64 WebSocket
  protocol instead of inventing incompatible messages.
- `packages/web-core/src/shared/dialogs/settings/settings/CliToolsSettingsSection.tsx`:
  add auth state/actions and an embedded login dialog/terminal.
- `packages/web-core/src/shared/components/XTermInstance.tsx` and terminal helpers:
  extract a reusable xterm view that accepts an endpoint while preserving the
  workspace terminal behavior.
- `packages/web-core/src/shared/lib/machineClient.ts`: open the CLI login socket
  with the same explicit host/relay options used by machine-aware HTTP actions;
  returning only an endpoint string would silently target the wrong machine.
- `crates/server/src/bin/generate_types.rs`: export new auth enums/fields; regenerate
  `shared/types.ts` through `pnpm run generate-types`.

## Implementation Steps

1. Add `CliToolAuthState`, catalog auth strategy metadata, safe probe parsing,
   effective-binary resolution, and tests. Initial support: `az`, `gam`, and
   `mgc-beta`; AWS and 1Password return unsupported explanations.
2. Refactor `PtyService` around a command-session primitive accepting executable,
   args, cwd, and a deliberately constructed environment. Preserve interactive
   shell creation as a wrapper; emit an exit event and ensure closing terminates
   the child.
3. Add `/api/cli-tools/{id}/login/ws` with signed WebSocket/machine routing,
   per-tool conflict locking, a 15-minute timeout, input/resize/cancel handling,
   output streaming, exit metadata, a final independent auth probe, and cleanup
   on every exit path.
4. Extend generated contracts and the machine client. Add a machine-scoped
   `openCliToolLogin` operation that passes `hostId`/`relayHostId` WebSocket
   options. Keep the command fixed by catalog id; never accept executable/args
   from the browser.
5. Extract/reuse the xterm endpoint component, add a CLI-login dialog to each
   eligible row, clickable URLs, Cancel/Retry, and refreshed status rendering.
6. Add backend lifecycle/probe/route tests and frontend rendered-DOM tests for
   action visibility and terminal states. Add English strings and propagate the
   new keys consistently to locale files according to existing i18n practice.
7. Regenerate types, format, and run focused Rust/frontend tests followed by
   workspace checks and lint.

## Data Model

No database or credential model is added. Catalog metadata is compile-time;
auth state is a transient probe result; active login sessions live only in the
local process and are keyed by `(machine instance, CliToolId)`. See
[`data-model.md`](data-model.md).

## Contracts

`GET /api/cli-tools` gains login support and auth-state fields. A new signed
WebSocket endpoint streams a fixed catalog login command with the existing
terminal input/resize vocabulary plus cancel, exit, and final status messages.
See [`contracts/cli-tool-login.md`](contracts/cli-tool-login.md).

## Constitution Check

- **I Clarity**: declarative per-tool strategies and typed states; no output
  heuristics hidden in the UI. ✅
- **II Test the contract**: acceptance criteria map to probe, PTY lifecycle,
  WebSocket, and rendered-DOM tests. ✅
- **III Small, reversible steps**: extends the shipped CLI catalog and terminal
  stack; no OAuth server or credential store. ✅
- **IV Shared boundaries**: terminal presentation stays reusable; `web-core`
  owns machine data and orchestration. Both local and remote host paths are in
  the test blast radius. ✅
- **VI Don't rebuild what shipped**: reuses `PtyService`, signed WebSockets,
  xterm, machine routing, and tool resolution. ✅

## Risks

- Vendor output/commands can drift. Keep commands declarative, pin tests to the
  catalog contract, and classify uncertain probes as `unknown`, never success.
- PTY cleanup currently relies on dropping handles. The refactor must explicitly
  terminate/wait for children and test disconnect/timeout paths.
- CLI output can contain sensitive values. Do not persist transcripts or log
  output; API errors carry typed summaries only.
- Host tools may differ from catalog versions. Probe and login the same resolved
  binary, and degrade unsupported command behavior to an actionable failure.
