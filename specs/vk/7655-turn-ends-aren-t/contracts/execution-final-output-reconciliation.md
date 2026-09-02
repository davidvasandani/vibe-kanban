# Contract: Local final-output reconciliation

Given a running local execution with non-empty normalized final assistant output
and no later tool/interaction/start activity:

| Executor terminal mechanism | Child after quiet bound | Result |
| --- | --- | --- |
| Explicit executor exit signal | alive | Reap owned group; persist `indeterminate` |
| Explicit executor exit signal | exited | Existing OS-exit path wins |
| Natural OS exit | alive | Preserve `running`; re-arm bounded check |
| Natural OS exit | exited | Existing OS-exit path wins |

Additional invariants:

- A normal executor signal always wins without waiting for the quiet bound.
- Final assistant output never directly produces `completed`.
- Later meaningful activity disarms/restarts final-output observation.
- The exit monitor uses the existing bounded completion-write retry and final
  status stream; no frontend-only state is introduced.
- Explicit user stop and restart work-preservation behavior are unchanged.
