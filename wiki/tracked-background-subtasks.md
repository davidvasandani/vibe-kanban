# Tracked background subtasks

Agent work that must outlive its caller belongs to a child issue and independent
workspace. Create the issue with `parent_issue_id` and `pipeline_ids: []`, then
call `start_workspace` with `resume_parent: true`. Select repository branches
explicitly. Supply `parent_session_id` when context cannot identify exactly one
session. Command pollers remain for commands; they do not run agent turns.

## Durable ownership and completion

The backend validates the actual remote parent/child relationship and records a
`background_subtasks` subscription before launch. The child is linked remotely
before setup starts. A repeated start for the same child issue returns the
accepted workspace; it does not launch another child after an ambiguous response.

A sequential reconciler persists the first observed outcome and hands it to the
existing issue-comment delivery ledger using a stable notification ID. It runs
after startup execution reconciliation. Persisted outcomes and ledger identity
make retries safe across restarts and remote outages.

Process exit alone is not a settled turn: cleanup or queued follow-ups may still
run. Use the persisted session boundary. Setup-only finalization must record a
boundary too, and a failed next-action launch must record one even when it never
created a coding process or exit monitor. An initial failed launch is also
settled when workspace creation and its coding process have both failed.

The reported result is a turn outcome, not proof of task success. Missing output,
failed creation, failure, archive and deletion remain explicit. Child commits
need explicit integration because the checkout is independent.

## Parent delivery and bounds

Pin the originating session, but do not grant the explicit-human-target consent
to revive stopped work. Reuse the shared dispatcher's boundary, queue, archive,
terminal-issue, uncertainty and duplicate guards. Multiple sessions on a parent
must never cause an arbitrary choice. A saved comment is distinct from delivery.

Capture continuation ancestry at registration: siblings started by the same
turn share a hop rather than consuming each other's budget as they finish.
Keep the existing two-hop continuation limit. Registration also atomically
limits five pending children per parent workspace, twenty starts per hour across
the local database, and prohibits nested background delegation.

Claude's `Agent` and legacy `Task` use a parameter (`run_in_background`), so deny
that parameter before bypass-mode auto-approval and give the supported workflow.
Foreground delegation remains available. Codex receives the workflow in its
per-thread developer notice; it must not end a turn expecting native background
subagents to wake Vibe Kanban.

## Contributed by

- vk/5de6-background-jobs
