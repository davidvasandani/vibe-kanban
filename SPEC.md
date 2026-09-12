# Technical specification: Polling workspace group

Task: vk/dc76-add-polling-to-w (workspace dc767899-0a2e-4454-9c38-5977913c508b).

Workspaces with active durable pollers currently appear under Needs Attention after an agent turn stops. Add a distinct Polling group to the workspace sidebar so ongoing monitoring is recognizable without opening the workspace.

Use the existing authoritative poller state, including waiting between executions, to classify workspaces. A workspace belongs to exactly one activity group. Preserve actual approval/question attention and active agent execution precedence; otherwise active polling takes precedence over unread completion, idle, and history grouping. Stopped/finished pollers must not keep a workspace in Polling. Preserve search, sorting, collapse state, selection, and existing non-polling behavior. Add translated labels and focused regression coverage for grouping and poller transitions. Scope is Vibe Kanban source only; no deployment changes are expected.

Implementation must trace current state plumbing before selecting a minimal change. Validate with required dependency installation, formatting, checks/lint, focused tests, and independent Codex review. Record reusable knowledge and merge a reviewed PR against the repository base branch.
