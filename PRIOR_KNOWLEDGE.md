# Prior knowledge: Polling workspace group

Task: vk/dc76-add-polling-to-w.

Searched the Vibe Kanban project knowledge base (`vibe-kanban/wiki/INDEX.md` and topic pages) for pollers, workspace activity, grouping, and carousel behavior. The knowledge base is populated; this stage made no changes to it.

- `wiki/vk-pollers.md`: A poller is a BackgroundHelper execution with `executor_action.typ.poller` metadata, not a separate persistence entity. Running means the lifetime of the periodic loop, including sleep between ticks. Completed/killed/failed executions are historical. The drawer reuses its existing execution stream; do not open a per-workspace socket just for a collapsed header.
- `wiki/kanban-items-state-and-activity-grouping.md`: Display preferences must not gate semantic activity signals. Grouping is derived and must not mutate issue status or sort order. Preserve user ordering inside groups.
- `wiki/workspace-carousel-view.md`: Sidebar attention historically means pending approval or unread activity after a run stops. Carousel has its own triage ordering; do not accidentally change it while adding the sidebar group.
- `wiki/INDEX.md`: Reusable topic pages use kebab-case names and a Contributed by task list. Update related topics and index together.

Planning implications: reuse list-summary data if available; otherwise extend the existing summary rather than N execution subscriptions. Add a separate polling predicate that does not hide approvals or mark activity seen. Test active-loop and terminal transitions and mutually exclusive grouping.
