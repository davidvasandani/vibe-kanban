# Prior knowledge — vk/e831-remember-filter

Searched vibe-kanban/wiki and docs/knowledge-base for board filters, preferences, and defaults. Relevant pages: wiki/kanban-board-filtering.md, wiki/new-issue-pipeline-defaults.md, wiki/workspace-sidebar-filtering.md.

- One pure filter function serves board and list layouts in both frontends. Preserve explicit-search visibility of sub-issues and the relative Me token.
- Defaults require one precedence resolver; an explicitly empty override must remain authoritative. Removing an override restores inheritance.
- Existing UI scratch persists project/view preferences; inspect its ownership and hydration before extending it. Host configuration is distinct from user settings.
- Avoid copying project tag identifiers to unrelated projects. Test positive and negative cases, hydration, and isolation.
