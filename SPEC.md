# Worker output replay gap recovery

Task: vk/1d23-vk-worker-output

## Problem and scope
Vibe Kanban reports “Worker output replay gap; execution state is indeterminate”. Determine the conditions producing this error and fix incorrect recovery behavior without misrepresenting a running, failed, or unknown execution as successful. Scope is Vibe Kanban source and, only if needed, its hosting configuration in homelab/modules/vibe-kanban-rebuild.nix.

## Requirements
- Trace worker output sequencing, replay retention, reconnects, and terminal evidence to identify the reproducible cause.
- Recover missing output when authoritative retained evidence permits it; preserve explicit indeterminate state when evidence is unavailable.
- Do not duplicate output, replay side effects, restart agent execution automatically, or infer success merely from a final response.
- Preserve cancellation, completion, and failure semantics across worker reconnects.
- Add focused regression tests covering the identified failure and adjacent sequence boundaries.
- Document verified causes, recovery limits, and operational implications.

## Delivery
Follow the requested ordered knowledge recall and SpecKit stages before implementation. Run repository setup, formatting, focused verification, and independent Codex review. Record reusable knowledge with this task identifier, commit, open and merge a pull request after checks. Implementation details will be refined using prior knowledge and source evidence.

## Refined scope after diagnosis and user follow-up
Recover outcomes from separately retained, identity-matching worker terminal evidence when replay is incomplete; never acknowledge missing events. Preserve indeterminate state without such evidence. Isolate Codex SQLite indices beneath the execution root so persistent indices cannot contain disposable rollout aliases. Keep shared transcript/auth files and MCP refresh semantics. Stale-rollout errors are a confirmed configuration-lifetime defect, but their causal connection to journal overflow is unproven.
