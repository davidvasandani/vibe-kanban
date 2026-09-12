# Implementation plan: Polling workspace group

1. Follow the workspace-scoped SpecKit commands in `.claude/commands`; their exact artifact directory is `homelab/specs/vk/dc76-add-polling-to-w/`. These documents concern only Vibe Kanban; application code remains in its own repo.
2. Refresh the constitution, specify behavior, resolve precedence, write the technical plan and dependency-ordered tasks, then analyze coverage before implementation.
3. Extend the existing workspace summary with active-poller presence, computed across the workspace's sessions from running BackgroundHelper processes with poller metadata. Regenerate shared types.
4. Map the summary into workspace UI data. Extend the shared sidebar partition with Polling, preserving pending approval and active run/creation precedence. Add a persistent collapsible section and all locale labels.
5. Add regression tests for query filtering, grouping precedence/exclusivity, terminal poller transitions, and rendered section behavior.
6. Install frozen dependencies; format; run focused tests, type generation/check, checks and lint. Address failures in scope and document environmental blockers precisely.
7. Run independent Codex review and resolve significant findings. Update and commit the Vibe Kanban wiki, then open and merge PRs for the service change and its required SpecKit artifacts.

Prior knowledge: see `../PRIOR_KNOWLEDGE.md`.
