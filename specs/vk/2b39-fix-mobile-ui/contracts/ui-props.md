# Contract: `@vibe/ui` list-view props

Additive and optional. Existing callers do not change, and omitting the prop
means desktop.

```ts
interface IssueListViewProps    { /* existing */ isMobile?: boolean }
interface IssueListSectionProps { /* existing */ isMobile?: boolean }
interface IssueListRowProps     { /* existing */ isMobile?: boolean }
```

`IssueListView` passes `isMobile` to every `IssueListSection`, and each
section passes it to every `IssueListRow`. `KanbanFilterBarProps.isMobile`
already exists. Its meaning now also covers the icon-only clear-filters
control.
