# Session execution exclusivity and peer isolation

## Incident and scope
On 2026-10-31 PLA-396 initial and follow-up executions overlapped in one VK session and worktree. Both resumed one Claude transcript; a later speculative peer message reached SWE-345. Investigate the reported race against current code. Changes are limited to Vibe Kanban and its own hosting configuration if required.

## Required behavior
- Admission of initial, UI follow-up, MCP run_session_prompt, reply, and automatic continuation must serialize per VK session, including the interval before process startup completes.
- A prompt arriving nine seconds into startup is queued and delivered after the active turn settles. Concurrent admissions cannot both create live coding executions. Queue claims and failure recovery must not lose or duplicate prompts.
- The server enforces at most one active coding execution per session; workers independently refuse a second live process for the session, including duplicate launches and resumed Claude session IDs. Guards survive cancellation and release only when processes are dead.
- VK-launched peer discovery and delivery cannot cross workspace boundaries, or require explicit opt-in with authenticated workspace/session attribution. Prefer disabling unsupported peer channels over relying on ambiguous display names.
- Derive any enabled VK peer identity deterministically from workspace and session IDs, stable across turns; document derivation and enforcement. Guidance treats unexpected same-worktree peers as an execution bug, never implicit delegation.

## Validation and delivery
Reproduce delayed startup with a controlled nine-second regression test, exercise concurrent admissions and worker duplicate launches, cover cleanup/retry and queue draining, and prove cross-workspace isolation and stable identity. Preserve existing supported continuation behavior. Run appropriate repository checks and independent Codex review before PR creation and merge. Record reusable findings in the project wiki with this task's identifier. Do not claim live deployment verification based solely on unit tests.
