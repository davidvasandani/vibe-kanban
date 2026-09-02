# Data Model: Turn completion clears the running composer

No persistent schema change is required.

## Existing entities

### ExecutionProcess

- Identity: one row per turn.
- Relevant state: `status` (`running`, `completed`, `failed`, `killed`,
  `interrupted`, `indeterminate`) and optional exit code.
- Authority: the database row drives the execution-process stream and composer
  activity projection.

### SpawnedChild lifecycle shape

- `exit_signal: None`: OS child lifetime is the turn's terminal evidence.
- `exit_signal: Some`: executor protocol supplies separate turn-terminal
  evidence; child liveness alone does not prove the turn is active.

This distinction is transient container state and is consumed by the exit
monitor. It does not add a stored field.

## State transitions

1. `running` + normal success signal/exit → `completed`.
2. `running` + failure evidence → `failed`.
3. `running` + explicit stop/recovery → existing `killed` or `interrupted`.
4. `running` + quiet final output + missing signal on a signal-driven executor
   → bounded reap → `indeterminate`.
5. `running` + quiet final output + live natural-exit child → remain `running`
   and re-evaluate later.

Only persisted terminal transitions clear the composer; conversation text does
not directly mutate UI activity.
