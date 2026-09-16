# Prior knowledge — vk/88c5-marking-an-issue

Read-only search of populated `vibe-kanban/wiki` and `vibe-kanban/docs/knowledge-base` for archive, Done, and issue status.

- `docs/knowledge-base/issue-status-side-effects.md`: single and bulk remote issue updates already archive linked remote workspaces transactionally for Done/Cancelled/Canceled. Existing frontend reconciliation is level-triggered in ProjectProvider and needs optional WorkspaceContext. It calls the existing local workspace update endpoint, deduplicates in-flight updates, retries on later snapshots, and never unarchives automatically. Investigate provider composition and missing links instead of duplicating this behavior.
- `wiki/kanban-items-state-and-activity-grouping.md`: workspace display preferences must not gate lifecycle semantics; status names are the established convention. Board drag-and-drop uses bulk updates.
- Existing Vibe Kanban constitution requires transactional remote side effects, reuse of shipped mechanisms, identity-scoped projections, and regression verification.

The requested task-specific SpecKit commands at workspace `.claude/commands` target `homelab/specs/vk/88c5-marking-an-issue/`; the commands checked into the service repo still name a different historical task. Use workspace commands and their exact paths for pipeline artifacts, with implementation confined to the Vibe Kanban service.
