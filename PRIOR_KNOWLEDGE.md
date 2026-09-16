# Prior knowledge — vk/fe2d-warn-when-starti

Searched the existing Vibe Kanban knowledge base (`vibe-kanban/wiki/INDEX.md` and topic pages) for issue identity, creation, workspace navigation and issue sections. The knowledge base is populated; no KB edits made during recall.

- `wiki/create-mode-repo-branch-defaulting.md`: workspace creation is the shared `CreateChatBoxContainer` create-mode flow, distinct from issue creation. Preserve repository selection and draft state.
- `wiki/kanban-issue-panel-sections.md`: issue workspace sections are shared UI components supplied by web-core containers. A section-header signal remains useful when collapsed. Rendered UI tests use remote-web's jsdom harness; set NODE_ENV=test.
- `wiki/workspace-creation-reliability.md`: creation is a durable asynchronous lifecycle. Advisory UI must leave its start and recovery contract intact.
- `wiki/workspace-navbar-breadcrumbs.md`: human issue identifiers are display text; UUIDs remain routing and matching authority.

Implementation implication: reuse project-synced workspace/PR records and shared create-mode UI; do not introduce title matching, a backend uniqueness constraint or another creation path. Preserve workspace identity when joining status/PR enrichment.
