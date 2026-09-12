# Prior knowledge: workspace creation failures

Task: vk/40fb-workspace-creati

Read-only recall searched vibe-kanban/wiki and docs/knowledge-base for workspace creation, worktrees, shared storage, and lifecycle failure.

- wiki/create-mode-repo-branch-defaulting.md: picker defaults include origin/main; all backend consumers must resolve local then remote names. Do not strip remote prefixes.
- wiki/self-hosted-deployment.md: source sync, build input, and service-owned Git registrations have distinct ownership; global worktree prune can corrupt or fail across ownership boundaries.
- docs/knowledge-base/clustered-workspace-execution.md: coordinator owns workspace administration; shared bare stores and node-identical paths are required. NFS path existence does not prove health. Preserve data on indeterminate ownership, and do not retry dispatch on another worker.
- .specify/memory/constitution.md, principle XXVIII: acknowledged lifecycle operations must outlive requests, have durable observable outcomes, and reconcile restart from positive evidence.

The exact reported message is emitted in crates/server/src/routes/workspaces/create.rs. These pages identify investigation boundaries, not proof of the current root cause. No knowledge-base pages were modified during recall.
- docs/knowledge-base/request-independent-workspace-creation.md: the current asynchronous implementation intentionally persists a safe generic failure and logs full detail; restart marks unfinished operations failed and must not replay partially committed startup.
