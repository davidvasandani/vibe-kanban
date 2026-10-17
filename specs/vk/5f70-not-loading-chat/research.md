# Research: The chat panel always finishes loading

## Investigation log (2026-09-26, ~04:05–04:20 UTC)

- Screenshots show app version `4df95bb` on iPhone, with two workspaces stuck on
  the conversation spinner at 9:04 PM PDT (04:04 UTC). The public origin
  `vibe.vasandani.dev` sits behind Cloudflare Access, which this worker cannot
  pass.
- Coordinator API at `172.16.100.102:3334`, measured directly from this worker:
  - `/api/health`, `/api/info`, and `/api/workspaces` answer in < 150 ms.
  - Workspace list streams: active snapshot 124 KB, archived 570 KB. `Ready`
    arrives in 0.3–2.6 s.
  - All 32 historic `normalized-logs` / `raw-logs` sockets for the Kindle
    workspace send `finished` within 29–257 ms, including a 1.18 MB log.
    The Field-access workspace: 1.5 MB in 22 ms.
- A managed headless browser at phone width (390 px iframe) on the same
  workspaces showed the spinner and then loaded (≈3 s alone, ≈20–40 s with two
  instances). So it was slow but not hung at test time.
- `/api/cluster/metrics` for think2 (coordinator): load 35–49 with only ~56%
  CPU busy. Top processes are `vibe-kanban` (65–93% CPU, 3.2 GB), several
  `git -C /srv/vibe-kanban-shared/cluster/workspaces/...`, and
  `kworker ... xprtiod` / `fscache`, i.e. NFS I/O wait.
- Code reading (see plan, "Root cause") found that the server closes cleanly
  without `finished` on a stream error, and that the client never settles on
  close and has no idle bound.

## Decisions

### Settle on close in the stream utility, not in each caller
- Chosen: `streamJsonPatchEntries` owns settlement, and callers keep their
  existing `onFinished` / `onError` handling.
- Rejected: wrapping each caller's promise in its own timeout/close watcher.
  That is two copies of the same rule, and it cannot see transport close
  without re-exposing the socket.

### Idle deadline, not a total deadline
- A total deadline would cut off a large history arriving slowly over a phone
  link. An idle deadline, reset on every message, only fires on silence.

### Idle deadline only for history
- A running agent's live stream may be silent for minutes while it thinks.
  That stream already has a reconnect/backoff loop.

### Leave the server's clean-close-on-error alone
- Sending an error frame or code 1011 would be more honest, but that is a
  wire-protocol change affecting every consumer, and the client must handle
  proxy/OS drops anyway. It is recorded as possible follow-up, not done here.

### No automatic retry of failed history
- See `clarifications.md` Q2.

## Dependencies
None added.
