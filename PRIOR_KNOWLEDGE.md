# Prior knowledge — vk/5de6-background-jobs

Searched `vibe-kanban/wiki`, `vibe-kanban/docs/knowledge-base`, and homelab's
knowledge directories read-only for background jobs, delegation, subtasks,
sub-issues, and pollers. The knowledge base is not empty.

- `vibe-kanban/wiki/agent-process-lifecycle.md`: agent turns are reaped at
  completion. In-process wakeup promises can silently abandon work. Persisted
  parent continuation was explicitly deferred alongside VAS-132 (Issue
  Background Tasks as Sub-Issues); do not assume it already exists.
- `vibe-kanban/wiki/vk-pollers.md`: command pollers are persistent background
  helpers, with an independent process group and shared concurrency budget.
  They run commands, not agent turns. Preserve this lifecycle. Blocking an
  unsupported tool requires a concrete supported replacement and instructions
  delivered to each executor; silent feature toggles are insufficient.
- `vibe-kanban/wiki/kanban-board-filtering.md`: Team view hides child issues
  unless searching; explicit searches reveal them. A hidden child issue is not
  necessarily a missing child issue.

Source inspection corroborates separate capabilities: MCP `create_issue`
accepts `parent_issue_id`; Codex normalization currently ignores collaboration
spawn/wait/completion events. Neither observation alone proves the screenshot's
root cause. Determine whether it shows CLI-internal delegation or a separately
tracked VK workspace before selecting the implementation.
