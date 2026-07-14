# Analysis: spec ↔ plan ↔ tasks cross-check

**Task**: `vk/5a2a-vk-cli-tool-logi`

## Requirement coverage

| Requirement | Covered by | Status |
|---|---|---|
| FR-1 catalog declarations | plan steps 1/6; T001/T006 | ✅ |
| FR-2 typed auth states | plan step 1; T004/T010 | ✅ |
| FR-3 actions by state | plan step 5; T013/T016 | ✅ |
| FR-4 effective binary/machine | plan steps 1/4; T001/T011 | ✅ |
| FR-5 interactive PTY | plan steps 2/3/5; T005/T008/T014 | ✅ |
| FR-6 clickable URLs | xterm WebLinks reuse; T012/T014 | ✅ |
| FR-7 cancel/cleanup/timeout | plan steps 2/3; T005/T007-T009 | ✅ |
| FR-8 per-tool conflict | plan step 3; T007/T009 | ✅ |
| FR-9 independent final probe | plan steps 1/3; T004/T008/T009 | ✅ |
| FR-10 bounded/no secret leakage | research + risks; T004/T009 | ✅ |
| FR-11 unsupported guidance | plan steps 1/5; T004/T013 | ✅ |
| FR-12 typed errors | contract; T008/T009/T016 | ✅ |
| FR-13 local/remote hosts | machine client and tests; T011/T016 | ✅ |

## Constitution check

- **Clarity**: typed catalog strategies and server-selected commands avoid UI
  heuristics and arbitrary shell strings. ✅
- **Test the contract**: unit, lifecycle, route, and rendered-DOM coverage are
  explicitly tasked before verification. ✅
- **Small/reversible**: no credential store, OAuth server, database, or install
  behavior change. ✅
- **Shared boundary**: reusable terminal presentation remains in shared UI;
  machine orchestration remains in `web-core`. ✅
- **Don't rebuild**: existing PTY, signed socket, machine transport, and xterm
  are generalized. ✅

## Gaps found and resolved

1. **Remote-host socket routing was underspecified.** `machineClient` HTTP calls
   pass explicit host/relay options, while `openLocalApiWebSocket` defaults to
   the current route host. A settings-selected host can differ. Plan step 4 and
   T011 now require a socket-opening operation carrying explicit machine scope.
2. **Graph probe remains vendor-version-sensitive.** T006 is a hard gate: enable
   Graph login only after confirming the pinned binary's safe status contract;
   otherwise report unsupported. This preserves the clarified no-false-success
   requirement without blocking Azure/GAM.
3. **PTY exit/cancellation is not present today.** T003/T005 require an explicit
   child handle/exit channel and termination tests; merely dropping the output
   reader is not accepted as cleanup evidence.

## Verdict

Artifacts are consistent and constitution-compliant after the remote-socket
correction. Cleared to implement in dependency order.
