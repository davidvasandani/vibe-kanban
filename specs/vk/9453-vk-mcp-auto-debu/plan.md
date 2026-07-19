# Technical Plan: VK MCP Auto Debug

**Feature dir**: `specs/vk/9453-vk-mcp-auto-debu/`
**Task**: `vk/9453-vk-mcp-auto-debu`
**Spec**: [`spec.md`](spec.md)

## Approach

Enhance the shared MCP settings result UI in
`packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`.
Failed assignment results will render their full backend diagnostic, expose
exact-copy behavior, and optionally create a local Vibe Kanban issue through the
existing project context mutation. The implementation should remain frontend
only: no MCP probe, OAuth, save/discard, backend route, schema, or generated type
change is expected.

Extract deterministic diagnostic and issue-description helpers into a small
shared lib module so Markdown fencing and issue payload content are covered by
unit tests without needing to drive the full settings dialog for every edge
case.

## Grounding

- `McpSettingsSection.tsx`
  - Owns shared MCP settings state, `testResults`, OAuth connect state, and the
    `TestResultDetails` presentation.
  - Currently truncates diagnostics with `truncate` in the attention result and
    secondary result list.
  - Uses `testKey(serverName, executor)` as the stable assignment-result key.
- `shared/types.ts`
  - `SharedMcpAssignmentTestResult` carries `server_name`, `executor`, and
    `result`.
  - `McpServerTestResult.error` is optional and must be treated as opaque text.
- `packages/web-core/src/shared/hooks/useProjectContext.ts`
  - `useProjectContextOptional()` exposes explicit current project context,
    issues, statuses, and `insertIssue`.
- `packages/web-core/src/shared/lib/routes/appNavigation.ts`
  - `goToProjectIssue(projectId, issueId)` supports a user-triggered success
    action after issue creation.
- `packages/web-core/src/i18n/locales/*/settings.json`
  - Settings strings are localized in seven locale folders:
    `en`, `es`, `fr`, `ja`, `ko`, `zh-Hans`, `zh-Hant`.

## Implementation Steps

1. Add pure helper module
   `packages/web-core/src/shared/lib/mcpDebugIssue.ts`.
   Include:
   - `mcpDiagnosticText(error, fallback)` returning `error` when it is a string
     with content, otherwise the localized fallback.
   - `markdownFenceFor(text)` choosing a backtick fence longer than any
     consecutive backtick sequence in `text`.
   - `buildMcpDebugIssueTitle({ serverName, executor, prettyExecutor })`.
   - `buildMcpDebugIssueDescription({ serverName, executorLabel, diagnostic })`.
2. Add unit tests in
   `packages/web-core/src/shared/lib/mcpDebugIssue.test.ts`.
   Cover exact multiline preservation, missing-diagnostic fallback, title
   content, and diagnostics that contain Markdown fence markers.
3. Refactor `TestResultDetails` in `McpSettingsSection.tsx` so failed results
   render a dedicated full diagnostic panel:
   - Use `whitespace-pre-wrap`, `break-words`, and a monospace diagnostic body.
   - Remove one-line truncation for failed diagnostics.
   - Keep `auth_required` and `unsupported` presentation/control behavior
     unchanged.
4. Pass assignment identity and action dependencies into failed result rendering:
   - `serverName`
   - `executor`
   - `projectContext` from `useProjectContextOptional()`
   - `navigation` from `useAppNavigation()`
   - per-result copy/debug state and handlers
5. Implement Copy:
   - Use `navigator.clipboard.writeText(diagnostic)`.
   - Store per-assignment success/error state keyed by `testKey`.
   - Render localized `aria-live` feedback.
   - On failure, show the error and keep the diagnostic unchanged.
6. Implement Debug issue creation:
   - Enable only when project context exists and has at least one status.
   - Sort project statuses by `sort_order` and use the first as the insertion
     status, matching the existing kanban create flow.
   - Compute top-of-column sort order with the existing `min(sort_order) - 1`
     pattern.
   - Call `insertIssue` with the contract in [`contracts.md`](contracts.md).
   - Await `persisted` and store the returned issue id in per-result state.
   - Disable the Debug button while that assignment key is creating.
   - Show mutation errors inline without altering diagnostics.
7. Render post-create success:
   - Keep Settings open.
   - Show a localized success message and a visible action that calls
     `navigation.goToProjectIssue(projectId, issueId)` when clicked.
   - Do not navigate automatically.
8. Update secondary failed results:
   - If multiple assignments fail, ensure each failed result can expose the
     same full diagnostic/copy/debug behavior instead of remaining a truncated
     one-line summary.
   - Preserve the current "one attention result plus remaining results" visual
     organization if possible, but do not leave failed diagnostics clipped.
9. Add English settings translations and matching keys for all other locale
   files using existing repository fallback convention where direct translation
   is not available. New keys should cover:
   - missing diagnostic fallback
   - copy button, copied, copy failure
   - debug button, creating, unavailable outside project, unavailable no status
   - issue created, open issue, issue creation failure
10. Extract failed-result view/action logic where needed and cover it with the
    existing Node Vitest harness. No rendered-DOM harness currently exists for
    this settings surface, so keep coverage focused on pure diagnostic, action
    availability, payload, duplicate-guard, error, and success contracts.
11. Run validation:
    - `pnpm --filter web-core test -- mcpDebugIssue`
    - relevant targeted MCP settings/lib tests
    - `pnpm run check`
    - `pnpm run lint`
    - `pnpm run format`
12. Perform an independent diff review before completion. Verify no generated
    files were edited manually and that existing MCP save/test/OAuth paths are
    untouched except for failed-result UI/action integration.

## Contracts

See [`contracts.md`](contracts.md). The feature uses the existing project issue
mutation and app navigation contracts; no new HTTP/API contract is introduced.

## Data Model

No persistent data model change. New transient frontend state is keyed by MCP
assignment test key:

- Copy status and error
- Debug creation status, error, and created issue id

Created issues use the existing issue model and store the debug context only in
the issue title/description.

## Constitution Check

- **I Clarity over cleverness**: failed-result behavior is explicit, helper
  functions are deterministic, and diagnostics remain opaque text.
- **II Test the contract**: unit tests cover exact diagnostic preservation,
  Markdown safety, and payload construction; targeted UI tests cover actions
  where feasible.
- **III Small, reversible steps**: reuse current MCP settings, project context,
  issue mutation, and navigation machinery.
- **IV Shared-component boundaries**: changes stay in `web-core` feature
  containers and use existing UI primitives from `@vibe/ui`.
- **V Remote mutations**: local frontend uses existing issue mutation contract;
  no `crates/remote` transactional mutation is added.
- **VI Don't rebuild what shipped**: no new clipboard abstraction, project
  selector, MCP probe path, or issue backend route.
- **X Dialogs hold provisional state**: MCP draft/save/discard state is not
  mutated by Copy or Debug actions.
- **XI Diagnostics are evidence, not decoration**: display, copy, and issue
  seeding preserve the exact backend diagnostic text.

## Risks

- Settings can be opened outside `ProjectProvider`; using the optional hook is
  required to avoid runtime errors.
- If project statuses are still loading or empty, issue insertion has no safe
  status id. The UI must explain that Debug is unavailable instead of guessing.
- Multiple failed assignments for one server currently have a compact secondary
  list. The implementation must not fix only the attention result and leave
  secondary failures clipped.
- Diagnostics may contain Markdown fences. The description builder must choose
  a longer fence dynamically.
- Browser clipboard APIs can fail because of permissions or insecure contexts;
  the error path needs visible feedback.

## Rollback

Remove the `mcpDebugIssue` helper/tests, the failed-result Copy/Debug UI and
state from `McpSettingsSection.tsx`, and the added settings translation keys.
Because no backend or database schema is changed, rollback is a frontend-only
revert.
