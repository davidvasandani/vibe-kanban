# Research: VK MCP Auto Debug

**Feature dir**: `specs/vk/9453-vk-mcp-auto-debu/`
**Task**: `vk/9453-vk-mcp-auto-debu`

## Findings

### MCP failed-result rendering

- `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
  owns shared MCP settings state, saved-server testing, OAuth connection, and
  current test-result presentation.
- `TestResultDetails` currently renders non-OK assignment results. It truncates
  the primary message with `truncate`, uses `title` for the full error, and
  reserves detailed controls for `auth_required`.
- The main server card shows one attention result plus a compact list for any
  additional non-OK assignment results. Additional results currently truncate
  their diagnostic as well.
- `SharedMcpAssignmentTestResult` includes `server_name`, `executor`, and
  `result`; `McpServerTestResult.error` is optional.

Decision: Extend only `failed` presentation. Keep `ok`, `unsupported`, and
`auth_required` behavior on the existing path so OAuth Connect remains
unchanged.

### Project context and issue mutation

- `packages/web-core/src/shared/hooks/useProjectContext.ts` exposes
  `useProjectContextOptional()`, `projectId`, `statuses`, `issues`, and
  `insertIssue(data)`.
- `ProjectProvider` supplies that context on the kanban project routes.
- Settings may open from project routes or non-project surfaces. Optional
  project context is therefore the correct availability boundary.
- Existing issue creation inserts at the top of a status column by computing
  `min(sort_order) - 1`.
- New issue payloads need a `status_id`. Existing create flows use a selected
  status; the debug flow has no status selector.

Decision: Use `useProjectContextOptional()` inside the settings section and
enable Debug only when a project context exists and has at least one status.
Insert the debug issue at the top of the first project status in the provider's
ordered statuses. If there is project context but no status is available yet,
disable Debug with a visible localized explanation.

### Navigation after creation

- `useAppNavigation()` exposes `goToProjectIssue(projectId, issueId)`.
- The requirement says Settings remains open and no automatic navigation occurs.

Decision: Store the created issue id in local result-level state and render a
button/link action that calls `goToProjectIssue(projectId, issueId)` only when
the user chooses it.

### Copy feedback

- No dedicated clipboard helper was found in the MCP settings surface.
- Browser clipboard writes are available via `navigator.clipboard.writeText`.

Decision: Implement a tiny local async Copy handler with per-result status:
idle/success/error. Render feedback in an `aria-live` region and leave the
diagnostic untouched on failure.

### Markdown safety

- Diagnostics are untrusted text and may contain backtick fences.
- Markdown code fences can be made safe by choosing a fence length longer than
  any consecutive backtick run in the diagnostic.

Decision: Extract pure helpers to a small `mcpDebugIssue.ts` module:
`diagnosticText`, `markdownCodeFence`, `buildMcpDebugIssueTitle`, and
`buildMcpDebugIssueDescription`. Unit tests cover missing diagnostics,
multiline diagnostics, and diagnostics containing fence markers.

## No Backend Change Expected

The existing `insertIssue` contract can create the needed local issue with
project id, status id, title, description, priority, sort order, dates, parent,
and metadata. No database, API, or generated shared type change is required.
