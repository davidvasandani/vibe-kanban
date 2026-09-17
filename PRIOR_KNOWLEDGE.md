# Prior knowledge: issue and workspace lifecycle
Task: vk/e464-issue-and-worksp

Searched vibe-kanban/wiki read-only for archive, status, follow-up and comment.
- issue-workspace-advisory.md: identify linked issues by project_id/issue_id, never workspace names. Active means archived=false, independently of execution activity. Local and remote workspace IDs differ.
- kanban-items-state-and-activity-grouping.md: statuses have no semantic kind; existing backend resolves In progress by project-scoped name. Preserve this convention.
- agent-process-lifecycle.md: queued follow-ups have a separate acceptance/dispatch path; changes must cover acceptance without relying only on agent process startup.
- browser-session-control-arbiter.md: workspace archive patches have cleanup subscribers; use existing model/event paths.
