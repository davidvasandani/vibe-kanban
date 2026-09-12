# Clarifications: Turn completion clears the running composer

## 1. Executor and placement scope

**Decision:** Treat the incident as an executor-neutral, placement-neutral
lifecycle defect. The screenshot establishes that the selected coding-agent
turn rendered final output while the composer still derived an active process,
but it does not prove local versus worker placement or safely identify a unique
executor protocol from the display label alone.

**Why:** The product contract is shared: the composer consumes authoritative
`ExecutionProcess` state, not executor-specific UI flags. Narrowing the user
requirement to an unproven mode could leave the same stale-running failure on
another supported path. Diagnosis and regression coverage may be focused on the
specific owning boundary repository evidence reveals, while verification must
preserve both local and clustered lifecycle invariants.

## 2. Meaning of final assistant output

**Decision:** Final output is a reconciliation trigger, not successful-exit
evidence.

**Why:** Output can precede process finalization. Directly changing the UI to
idle from transcript content would hide a genuinely live or stuck process and
remove its cancellation affordance. Any fallback must remain bounded and use
positive owner-specific liveness evidence before recording a truthful terminal
or indeterminate outcome.

## 3. Expected recovery time

**Decision:** Normal terminal evidence should update the open composer as soon
as it is persisted and streamed. Lost evidence must use the repository's
existing bounded reconciliation policy; this task does not introduce an
independent UI timeout.

## Remaining questions

None.
