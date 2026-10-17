# Clarifications: The chat panel always finishes loading

No answers were supplied with the clarify stage. Both open questions were
resolved from measurements and existing behaviour.

## Q1. What idle deadline applies to history fetches?

**Decision: 30 seconds without any message, reset on every message.**

- The server sends nothing until it has re-read and re-normalized the whole raw
  log. On 2026-09-26, with think2 at load ~49 (mostly NFS I/O wait), the first
  message for all 32 turns of the Kindle workspace arrived in 29–221 ms, and
  the largest (1.18 MB) finished in 257 ms. 30 s leaves more than 100× headroom
  for a much worse moment while still bounding a dead socket to something a
  person will wait through.
- Resetting on every message means a large history that is still arriving over
  a slow mobile link is never cut off; only silence is.
- Live streams are exempt (FR-5), so a silent running agent is unaffected.

## Q2. Are failed turns retried automatically during the initial load?

**Decision: on demand only, through the existing "load earlier" control.**

- The failure mode shows up when the coordinator is already struggling.
  Automatic retries would add normalization work at the worst moment.
- `loadProcessesInOrder` already skips and counts failed turns, and a turn that
  is not loaded is already reported by `hasUnloadedHistoricProcesses`. So the
  "load earlier" control appears, and using it re-requests the failed turn and
  surfaces `loadEarlierError` if it fails again. No new UI is needed (III).
