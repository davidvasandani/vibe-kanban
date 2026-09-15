# Implementation plan: Slack MCP in VK

1. Complete SpecKit constitution, specification, clarification, plan, tasks and
   analysis in the workspace command's designated homelab spec directory.
2. Trace `crates/executors/src/shared_mcp_config.rs`, `mcp_config.rs`, Codex
   launch configuration and cluster MCP snapshots. Inspect only redacted live
   Slack configuration and perform bounded initialize/tools-list probes.
3. Record the reproduced failure in the task research artifact. Select the
   smallest fix within VK or `homelab/modules/vibe-kanban-rebuild.nix`; stop for
   clarification if another service needs changes.
4. Implement the fix and regression coverage together. Preserve custom entries,
   credentials, settings authority and the pinned attachment-capable fork.
5. Install worktree dependencies and run appropriate targeted tests and required
   formatting. Verify the failing boundary using read-only live checks where
   available; distinguish static checks from runtime recovery.
6. Run independent Codex CLI review, address significant findings, and recheck.
7. Commit reusable knowledge with this task identifier and update the index.
8. Open and merge pull requests against the actual base branches, verifying CI
   and reporting deployment/runtime limitations explicitly.

Inputs: `SPEC.md`, workspace `PRIOR_KNOWLEDGE.md` and repository instructions.
