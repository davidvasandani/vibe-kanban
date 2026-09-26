# SPEC: Chat panel never finishes loading (vk/5f70-not-loading-chat)

Vibe Kanban only. No homelab or deployment change.

Feature artifacts: `specs/vk/5f70-not-loading-chat/` (spec, clarifications,
plan, research, data model, contract, tasks). Constitution principle: XL.

## Problem

On an iPhone (version `4df95bb`, through `vibe.vasandani.dev`), two
workspaces showed the conversation spinner indefinitely at 04:04 UTC on
2026-09-26. The chat box rendered, but no messages ever appeared.

## Root cause

The chat's initial load fetches each recent completed turn over a WebSocket
(`/api/execution-processes/{id}/normalized-logs/ws`, or `raw-logs/ws` for
scripts) and waits for `{"finished": true}`:

- **Server** (`crates/server/src/routes/execution_processes.rs`): on a
  log-stream error, both handlers `break` and `socket.close()`, which is a clean
  1000 close **without** `finished`.
- **Client** (`packages/web-core/src/shared/lib/streamJsonPatchEntries.ts`):
  the waiter settles only on `finished` or a transport `error` event. `close`
  was ignored, and nothing timed out a silent socket.
- `useConversationHistory.loadEntriesForHistoricExecutionProcess` wraps this in
  a promise. `loadProcessesInOrder` awaits `Promise.all` over a slice, so a
  single stuck fetch keeps `ConversationList.loading` true forever.

Evidence: at the time, the coordinator (think2) was at load 35–49 with ~56% CPU
busy, i.e. NFS I/O wait (`xprtiod`/`fscache` kworkers, many `git -C` processes
on shared workspaces). That is when a raw-log read can fail. Proxies
(Cloudflare) and iOS dropping the socket produce the same client state. Direct
LAN tests afterwards loaded fine (all 32 Kindle-workspace log sockets finished
in < 300 ms), so the trigger is intermittent. The client defect is
deterministic and is covered by tests.

## Requirements

- R1: A history fetch ends as success (entries + `finished`) or failure. A
  close without `finished`, even a clean one, is failure.
- R2: A history fetch silent for 30 s fails. The timer resets on every
  message, so slow-but-arriving logs are never cut off.
- R3: Exactly one outcome per fetch. Signals after settlement are ignored.
  Closing a fetch deliberately reports nothing.
- R4: A failed turn is skipped (existing `loadProcessesInOrder` behaviour).
  The initial load completes, and the turn stays reachable through
  "load earlier".
- R5: Live (running) streams have no idle deadline. A close without
  `finished` now rejects and is retried by the existing
  `loadRunningAndEmitWithBackoff`.

## Non-goals

- Changing server stream termination or the wire protocol (possible
  follow-up: send an error close code on log-read failure).
- Workspace-list / execution-process streams (`useJsonPatchWsStream`), which
  already reconnect on close.
- Reducing NFS pressure on think2.

## Acceptance

- `packages/web-core/src/shared/lib/streamJsonPatchEntries.test.ts`: 8 cases
  (finished; clean close without finished; error+close; idle expiry; steady
  messages; open never completes; caller close; no deadline). Three of them
  fail on the pre-fix code and all pass with the fix.
- web-core vitest (539 tests), `tsc` for all frontends, frontend lint, and
  prettier all pass.
