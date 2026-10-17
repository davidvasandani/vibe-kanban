# Contract: Plan-Reveal Transition

Given an incoming timeline update type, latest entry, and previously revealed
plan patch key:

| Latest entry | Previous key | Effective type | Next key |
|---|---|---|---|
| Not `ExitPlanMode` | any | incoming type | unchanged |
| `ExitPlanMode` key A | null | `plan` | A |
| `ExitPlanMode` key A | A | incoming type | A |
| `ExitPlanMode` key B | A | `plan` | B |

The transition is deterministic, has no time dependency, and does not mutate
the entry snapshot.
