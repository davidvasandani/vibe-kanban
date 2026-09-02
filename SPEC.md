# Technical Spec: Stop control clears when a Vibe Kanban turn ends

**Task:** `vk/7655-turn-ends-aren-t` — “Turn Ends aren't Stopping Vk UI”

## Problem

The workspace chat composer continues to show the running-turn Stop control and
spinner after the agent has emitted its final response and the turn is over.
This leaves the composer in a false running state and makes it appear that Vibe
Kanban is still doing work or waiting for a stop operation.

The screenshot shows a completed assistant response while the composer footer
still renders `Stop` with the running spinner. In the current UI,
`SessionChatBoxContainer` derives that control from
`isAttemptRunningVisible`, which in turn is true while any visible agent or
workspace-script execution process streamed for the selected session retains
the `running` status.

## Goal

Make the authoritative execution-process lifecycle transition reach the VK UI
when a turn terminates, so the composer returns from the running Stop control to
its idle Send/continue state without a reload or manual stop.

## Functional requirements

1. When a coding-agent turn reaches a terminal outcome, its execution process
   must stop being reported as `running` to the selected session's execution
   process stream.
2. The composer must stop rendering the running Stop control after the relevant
   terminal process update is received.
3. All terminal outcomes (`completed`, `failed`, `killed`, `interrupted`, and
   `indeterminate`) must be treated as non-running; active coding-agent and
   workspace-script processes must remain cancellable.
4. Completion must reconcile correctly across the local and clustered/remote
   execution paths used by the Vibe Kanban service. Any deployment change must
   be limited to `homelab/modules/vibe-kanban-rebuild.nix` and only if required
   by the source fix.
5. Existing conversation output, queued follow-ups, approvals, dropped-process
   filtering, session switching, and reconnect behavior must not regress.

## Technical approach

Trace the process from executor termination through persistence and the
execution-process JSON Patch WebSocket stream to
`ExecutionProcessesProvider`. Identify where the terminal status is missing,
delayed, or hidden. Fix the earliest authoritative lifecycle boundary rather
than inferring completion from rendered conversation text. Preserve the UI's
existing `hasRunningAttempt` semantics and add focused regression coverage at
the layer where the stale running state originates, plus UI/provider coverage
where useful.

The detailed design will be refined after the required project-knowledge search
and SpecKit clarification/research stages.

## Acceptance criteria

- A naturally completed agent turn changes the composer from Stop/spinner to
  its idle action without refreshing the page.
- Failed, killed, interrupted, and indeterminate turns likewise do not leave a
  stale running Stop control.
- A genuinely running agent/setup/cleanup/archive process still presents Stop.
- Focused automated tests reproduce the stale-state scenario before the fix and
  pass afterward.
- Relevant frontend and/or Rust checks, formatting, and the independent Codex
  diff review pass with no significant findings.

## Scope

In scope: Vibe Kanban source and, only if necessary for deploying that source,
`homelab/modules/vibe-kanban-rebuild.nix`.

Out of scope: changes to any other service, broad redesign of the chat composer,
or treating assistant message text as the source of truth for process state.
