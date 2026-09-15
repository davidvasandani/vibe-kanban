# Implementation plan: Require bounded pollers

1. Execute the repository SpecKit command instructions, refreshing applicable constitution principles and writing feature artifacts for this workspace.
2. Extend PollerSpec with optional stop_command and timeout_secs; validate new requests, retain legacy JSON compatibility, and compile bounded shell execution with stop checks and a whole-process deadline.
3. Carry the fields through HTTP/MCP create and list contracts, generated types, and existing poller UI projections. Update agent instructions.
4. Add process-level tests for stop success/failure, hanging commands, deadline cleanup, invalid requests, and legacy metadata. Run install, generation, formatting, and relevant checks.
5. Run an independent Codex diff review; resolve significant findings and recheck.
6. Update and commit poller knowledge with this task id, then open and merge a pull request once verified.

Read `PRIOR_KNOWLEDGE.md` for existing lifecycle and contract constraints. Application changes are confined to Vibe Kanban. Workspace-provisioned SpecKit commands place their Vibe Kanban feature artifacts in `../homelab/specs/vk/bd71-require-all-poll/`.
