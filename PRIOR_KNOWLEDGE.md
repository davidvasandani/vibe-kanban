# Prior Knowledge: VK MCP Auto Debug

Task: `9453-vk-mcp-auto-debu`

The Vibe Kanban project knowledge base was searched through
`docs/knowledge-base/INDEX.md` and its topic pages for MCP testing, settings,
diagnostics, issue creation, project context, and clipboard behavior.

## MCP connectivity testing

Source: `docs/knowledge-base/mcp-connectivity-testing.md`

- Vibe Kanban normally writes agent-native MCP configuration; the explicit test
  flow is the exceptional client path that probes saved entries on demand.
- Probe results deliberately distinguish `ok`, `failed`, `auth_required`, and
  `unsupported`. Auto-debug UI must attach only to genuine failures and must not
  collapse authentication or unsupported results into the same workflow.
- `McpServerTestResult.error` carries actionable transport/process diagnostics.
  The frontend currently indexes results by server/executor and clears stale
  state after configuration changes; the enhancement must retain those rules.
- The probe may include multiline stderr and transport errors. Preserving the
  exact string is important because stdio stderr is intentionally attached to
  failures and timeouts are bounded at the probe layer.

## Shared MCP configuration

Source: `docs/knowledge-base/shared-mcp-configuration.md`

- Shared tests read saved native entries and return assignment-level results
  keyed by logical server name and base executor.
- The MCP inventory is a read-oriented management surface with explicit testing
  and detailed per-assignment state. Debug actions should not mutate the MCP
  draft, assignment list, redacted credential snapshot, or save/discard state.
- Assignment compatibility and secret hydration are established contracts. This
  task should remain a diagnostic/issue-creation UI path rather than modifying
  materialization or connection behavior.

## MCP OAuth Connect flow

Source: `docs/knowledge-base/mcp-oauth-connect.md`

- `auth_required` results drive a specialized Connect workflow with popup,
  polling, snapshot refresh, and re-test behavior.
- Auto-debug must leave that workflow intact and avoid showing Debug as a
  substitute for Connect on authentication challenges.
- Error text can contain remote or process-controlled content. Existing OAuth
  guidance treats it as untrusted display data; issue descriptions should pass
  it as inert text/Markdown content, not interpolate it into HTML.

## External integration issue creation

Source: `docs/knowledge-base/remote-external-integrations.md`

- The knowledge base documents server-side issue creation for remote external
  integrations, including explicit project mapping and the shared issue
  repository, but it does not document a reusable local-settings frontend
  mutation for creating an issue.
- This is a useful boundary warning: the implementation must inspect and reuse
  current local project/issue infrastructure rather than copying the remote
  integration path or inventing an arbitrary project selection rule.

## Knowledge gaps to resolve during planning

- No topic page currently records how a global settings screen obtains the
  active local VK project or navigates to a newly created issue.
- No topic page records a standard copy-to-clipboard feedback component.
- The implementation plan must therefore verify provider availability, mutation
  semantics, and existing UI primitives directly in the codebase.

## Implications for specification and planning

1. Enhance only `failed` assignment results; preserve all other status flows.
2. Treat the backend-provided diagnostic as an opaque exact string for display,
   copy, and issue context.
3. Keep the debug mutation orthogonal to MCP configuration and OAuth state.
4. Resolve project identity explicitly from existing local app context; never
   default to an arbitrary project.
5. Escape/fence diagnostic Markdown robustly and add focused tests for multiline
   content and mutation failure states.
