# Contracts: VK MCP Auto Debug

**Feature dir**: `specs/vk/9453-vk-mcp-auto-debu/`
**Task**: `vk/9453-vk-mcp-auto-debu`

## UI Contract

Failed MCP assignment results render a diagnostic panel with:

- Full diagnostic body: exact `result.error` when present, otherwise localized
  fallback text.
- Copy button: writes exactly the diagnostic body to the clipboard.
- Copy feedback: visible and announced success or failure.
- Debug button: visible and enabled only when the settings dialog is rendered
  inside an active local project context with an available project status.
- Unavailable Debug explanation: visible when no active project context exists,
  or when the project context has no status available for issue insertion.
- Creation feedback: per-result pending, error, or success state.
- Success action: a visible "open issue" action after creation; no automatic
  navigation.

Non-failed result contract:

- `ok` returns no detail panel.
- `auth_required` keeps the Connect, loopback, manual-code, and connect-error
  controls.
- `unsupported` keeps current non-debug display behavior.

## Issue Creation Contract

Use the existing `ProjectContextValue.insertIssue` mutation with:

```typescript
{
  project_id: projectId,
  status_id: firstProjectStatusBySortOrder.id,
  title: buildMcpDebugIssueTitle({ serverName, executor }),
  description: buildMcpDebugIssueDescription({
    serverName,
    executor,
    diagnostic,
  }),
  priority: null,
  sort_order: minSortOrderInStatus - 1,
  start_date: null,
  target_date: null,
  completed_at: null,
  parent_issue_id: null,
  parent_issue_sort_order: null,
  extension_metadata: null,
}
```

`firstProjectStatusBySortOrder` is the lowest-`sort_order` project status.
`minSortOrderInStatus` is computed from `projectContext.issues` filtered to that
status. If no issues exist there, use `0` and insert at `-1`, matching existing
kanban create behavior.

## Created Description Shape

The description is deterministic Markdown:

`````markdown
Investigate this saved MCP server connectivity failure.

MCP server: <serverName>
Executor: <pretty executor>

Diagnostic:
````text
<exact diagnostic>
````

Instructions:
- Reproduce the MCP assignment test failure.
- Identify the root cause without changing secret-redaction or OAuth semantics.
- Implement the smallest fix that preserves existing MCP assignment behavior.
- Run relevant tests and report the root cause, fix, and verification.
`````

The actual fence length must be longer than the longest backtick sequence inside
the diagnostic.

## State Contract

Per assignment key (`testKey(serverName, executor)`) track:

- `copyStatus`: `idle | success | error`
- `copyError`: optional localized/browser error detail
- `debugStatus`: `idle | creating | success | error`
- `debugIssueId`: created issue id after success
- `debugError`: optional failure message

The `creating` state disables repeated Debug submissions for that result.
Failure states never remove or change the diagnostic text.
