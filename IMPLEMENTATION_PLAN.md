# Implementation plan

1. Persist archive trigger/provenance and explicit-unarchive acknowledgement without changing existing workspace row decoding unnecessarily.
2. Route merge paths and remote reconciliation through one guarded automatic admission method. Retain blocked intent for periodic retry; compare explicit-unarchive acknowledgement at the write boundary.
3. Atomically reactivate the local workspace when accepting a valid wake-up. Preserve remote completion intent and existing delivery machinery.
4. Propagate originating session identity alongside workspace identity and use it for omitted MCP targets, without guessing among sessions.
5. Expose archive provenance in workspace listing and UI, and improve unavailable-worktree errors.
6. Add regression tests for holds, release, races, context defaults, scheduling, and diagnostics. Run required setup, formatting, and relevant validation.
7. Run independent Codex review, fix findings, update knowledge pages, commit, open PR, verify checks, and merge.

Detailed SpecKit artifacts follow in specs/vk/e8d6-wake-ups-an-auto/. Prior knowledge is recorded at ../PRIOR_KNOWLEDGE.md. Only Vibe Kanban service files are in scope.
