# Workspace affinity migration

Contributing tasks: `9a64-vk-workspace-aff`, `61a3-server-affinity`

Workspace affinity is both a placement policy and a resolved worker. Keep those concepts
separate: automatic placement may already resolve to the worker an operator later pins,
which changes policy without requiring a stop/restart.

Live migration is a coordinator-owned durable operation, not a client sequence. Claim one
operation per workspace, persist the source execution before stopping it, revalidate after
claiming, and use deterministic continuation execution identity. A stale claim may resume
only from durable evidence; an unproven stop leaves placement unchanged, while a failed
restart after reassignment is reported as a precise stopped-on-new-affinity outcome.

Retries must replay completed outcomes before evaluating current state. Dispatch creation
must tolerate the same execution identity while rejecting a mismatched worker or request
digest. Retain operation identity for transport ambiguity, but discard it after a conclusive
API error so a corrected retry is a new operation.

Placement controls must use the same online, mount-health, lease, and executor-capability
eligibility rules as the scheduler. Bulk workspace summaries should carry resolved affinity
to avoid per-row requests, and every memo/cache dependency that renders affinity must update
when placement changes.

Collapsed affinity UI should read that bulk summary rather than keep the detail
container mounted or issue a label-only request. Resolve display context in the same order
everywhere: assigned worker hostname, requested worker hostname, then placement-kind copy.
Keep dynamic header metadata in its own bounded, truncating flex item so the disclosure
caret remains usable. In the expanded body, align labels and controls with a two-column grid
(`auto` plus `minmax(0, 1fr)`) instead of independent `justify-between` rows; this preserves
label/value association and lets selectors shrink without overflow.
