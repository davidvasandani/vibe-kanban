# Background jobs must use tracked subtasks

Task: `vk/5de6-background-jobs`

## Problem
The supplied SWE-320 screenshot shows an agent ending its turn with “Still
waiting on the wiring map. I'll pick the work back up as soon as it reports.”
The user reports that background jobs are not using subtasks. Work delegated
outside Vibe Kanban's tracked lifecycle can disappear when a turn exits and
cannot reliably resume the parent.

## Intended behavior
Agent-delegated background work must be represented by a tracked child task,
with a visible relationship to its parent and truthful running/completed/failed
state. A parent must not promise automatic continuation unless completion is
actually delivered through a supported continuation mechanism.

## Scope and investigation
Only Vibe Kanban is in scope. Inspect executor launch instructions, delegation
tools, child issue/workspace relationships, and completion delivery before
choosing the smallest coherent fix. Distinguish delegated agent work from
command pollers and persistent process helpers. Preserve existing command
poller behavior.

## Acceptance
- Reproduce the unsupported delegation path implicated by the screenshot.
- Delegated agent work uses the supported child-task lifecycle.
- Completion and failure are visible and do not silently abandon the parent.
- Cover the changed lifecycle or enforcement boundary with regression tests.
- Run independent Codex review, record reusable knowledge, and merge a PR.

## Clarification
The user confirmed both tracked subtasks and automatic parent resumption.

## Final design
Use existing child issues and independent workspaces. `start_workspace` gains
opt-in `resume_parent`; the backend persists ownership before launch, links the
child before setup, and records an immutable outcome. Existing durable comment
delivery resumes exactly the originating session at a confirmed boundary.
Setup-only failures, turn failures, cancellation and restart recovery report
truthful outcomes. Stopped/archived/terminal parents are not revived.

Delegation is bounded to five pending children per parent, twenty starts per
hour globally, one depth, and the existing two-hop continuation budget.
