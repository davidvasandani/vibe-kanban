# Research: `e524-debug-stuck-sess`

No new dependency. Incident evidence is in `technical-spec.md`; prior
project knowledge is in `prior-knowledge.md`.

## Decision 1: what distinguishes a row that is safe to terminalise

**Chosen**: null `pgid` and no worker job, in addition to no in-memory
owner.

**Why**: `stop_execution` deliberately leaves an ownerless row `Running` so
the next boot can snapshot its uncommitted work. That only matters if a
process ran. `pgid` is written at local spawn
(`local-deployment/src/container.rs`, after `spawned.child.id()`) and never
cleared, and a worker job row is written before any worker dispatch, so
their joint absence means nothing ran. The project already uses the same
evidence to decide whether a turn launched (`wiki/issue-comment-delivery.md`).

**Rejected**: terminalising every ownerless row. It would hide a launched
row from boot recovery and lose work.

## Decision 2: how an abandoned start is detected

**Chosen**: an RAII guard around the row-creation-to-dispatch window.

**Why**: the journal cannot say whether the request future was dropped or
hung. A drop guard handles the drop deterministically, with no timer. The
registry it maintains also lets Stop tell a live start from a dead one.

**Rejected**:
- *Run follow-up starts in an owned task* (as workspace creation does). It
  is the fuller fix for a dropped request, but the start borrows `&self`,
  `&Workspace` and a task-local admission scope, so it is a larger change
  with its own risks. It also does nothing for the stop path.
- *Periodic sweeper for ownerless rows.* It adds a coordinator-side
  timeout that terminalises work without a user asking, which the
  constitution (XVIII, LVI) steers away from.

## Decision 3: the hung-start case

**Chosen**: a 120 s bound, consulted only when a user presses Stop.

**Why**: without it, a start that hangs rather than drops stays "in
flight" forever and Stop could never clear it. Pre-dispatch work normally
takes seconds.

**Revised after Codex review**: the stop also revokes the start's right to
launch, at an explicit launch boundary, so a hung start that resumes
cannot dispatch under a terminal row. See Decision 5.

## Decision 4: runtime query for the conditional update

**Chosen**: `sqlx::query` rather than `sqlx::query!`.

**Why**: no offline-cache regeneration, and the model already mixes both.
The statement is a single fixed `UPDATE` with bound parameters.

## Decision 5: an explicit launch boundary

**Chosen**: a phase on the registry entry, with stop and launch as
mutually exclusive transitions under the entry lock.

**Why**: Codex review found three races in the first version. A stale
start could be stopped and then launch anyway. A caller dropped after a
local spawn but before the `pgid` write had its row failed, hiding it from
boot-time work preservation. A stop that arrived while `create` was still
awaiting could end a row that then launched. All three come from treating
"no durable launch evidence yet" as "cannot launch". Durable evidence is
written after the launch, so an in-memory boundary crossed before it is
the only place the two can be made exclusive.

**Rejected**: a durable "launching" marker column. It needs a migration
and still leaves the question of who clears it after a crash, which boot
orphan cleanup already answers for `Running` rows.
