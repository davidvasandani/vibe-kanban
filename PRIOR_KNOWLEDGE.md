# Prior Knowledge: Settings Pipeline Management

The project knowledge base is populated in both `wiki/` and
`docs/knowledge-base/`. No existing page documents the pipeline-file management
API or a Settings pipeline editor, so this feature is new UI territory. The
following existing guidance constrains the implementation.

## Pipeline identity and catalog refresh

Source: `wiki/task-pipeline-block.md` (`vk/77eb-vk-pipeline`).

- Pipeline ids, not display names, are the stable identity. Display names are
  explicitly non-unique, so the Settings file list and editor selection must
  key by file id.
- The task-create `PipelineSection` consumes the React Query-backed pipeline
  catalog. Every create, write, delete, or reset action must invalidate that
  catalog as well as the new status/raw queries so task composition does not
  continue using stale stage definitions.
- A pipeline edit can change prompt fragments that existing task descriptions
  use for best-effort reverse parsing. The editor should preserve raw content
  exactly and avoid implicit rewrites; the server-validated TOML is
  authoritative.

## Machine-aware Settings requests

Source: `docs/knowledge-base/cli-tool-oauth-login.md`
(`5a2a-vk-cli-tool-logi`, `6777-aws-sso-config-i`).

- Host-specific Settings features must pass the selected host/relay scope
  explicitly. Calling the right URL without selected-host routing can silently
  operate on the UI machine instead of the machine selected in Settings.
- Pipeline query keys therefore need to include host identity, and every
  pipeline API method used by Settings must go through the host-aware request
  transport.

## Settings modal lifecycle and draft ownership

Sources: `docs/knowledge-base/shared-mcp-configuration.md` and
`docs/knowledge-base/mcp-connectivity-testing.md`.

- The Settings dialog is mounted outside route content and NiceModal can reuse
  mounted components. Editable fields must be deliberately seeded when the
  selected pipeline or host changes; stale drafts must not leak across files,
  hosts, or modal opens.
- Management dialogs should own provisional form state and keep server state in
  the existing query layer. Cancel/close must not persist a draft.
- React-only state guards can reset across modal lifecycle boundaries. Pipeline
  mutations should rely on TanStack Query pending state and awaited requests,
  with controls disabled during mutation, rather than a transient success flag
  as the only duplicate-action guard.

## Reusable validation pattern

Source: `docs/knowledge-base/cli-tool-oauth-login.md`.

- Prefer focused pure-state tests plus frontend type checking and an independent
  diff review. For this task, the highest-value pure logic is pipeline-id
  validation, error location formatting, and list selection after mutations.
- Preserve structured backend errors instead of flattening them prematurely.
  `PipelineParseError` already carries an optional 1-based line and column,
  which the editor and malformed-file list should display directly.

## Knowledge gap to close after shipping

If implementation confirms reusable conventions for file-backed Settings
editors—especially host-scoped query keys, raw draft validation, mutation
invalidation, or bundled/default reset discoverability—record them in a focused
knowledge page and add task id `3a97-no-frontend-for` to its contribution list.
