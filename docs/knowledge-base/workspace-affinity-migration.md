# Workspace affinity migration

Contributing tasks: `9a64-vk-workspace-aff`, `61a3-server-affinity`,
`vk/d80e-fix-the-spacing`

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

Compact control sections must also opt out of the right drawer's remaining-height
sharing. `CollapsibleSectionHeader` has three distinct sizing contracts: opted-in
expanded panels use `flex-1 min-h-0`, an explicit intrinsic mode uses `flex-none
h-auto`, and the omitted/default mode retains legacy `h-full min-h-0` behavior.
Passing a false fill flag is therefore not enough to make a section intrinsic.
Express the policy per section in `RightSidebar`, use intrinsic sizing for Server
Affinity, and keep content-heavy Git, logs, preview, metrics, terminal, and notes
panels flexible. Test the real primitive's rendered root classes: a prop-forwarding
mock cannot detect a full-height default that recreates the visual gap.
