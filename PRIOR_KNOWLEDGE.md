# Prior Knowledge: Three rollout loose ends

Task: `vk/94c0-three-loose-ends`

Searched `docs/knowledge-base/`, its `INDEX.md`, and the legacy `wiki/` pages
for i18n consistency, API envelope errors, MCP caller behavior, Codex config,
strict validation, and verification boundaries.

## Directly relevant knowledge

### `wiki/vk-pollers.md`

This is the primary design record for items 2 and 3.

- A typed rejection is not the MCP contract. `ApiResponse::error_with_data`
  omits `message`, while the MCP client surfaces only that field, so tests must
  assert the message on the response envelope.
- Agent-facing denials must state both the problem and corrective action; an
  `Unknown error` prevents self-correction.
- Codex `features.unified_exec=false` is a verified exact config identifier.
  Codex's deserializer accepts unknown fields unless strict config is requested,
  so a plausible-looking misspelling can be completely inert.
- Authoritative Codex identifiers must be checked against the source/artifact
  corresponding to the pinned executable, not inferred from stale types or UI.

### `docs/knowledge-base/worktree-formatting-prerequisites.md`

- A fresh worktree must run `pnpm install --frozen-lockfile` before repository
  formatting or frontend verification.
- Run the dependency preflight before mutating formatting stages and verify the
  package-local frontend tools rather than assuming a root shim.

### `docs/knowledge-base/prompt-driven-agent-pipelines.md`

- Pipeline prompts are executable contracts and their required artifacts and
  order must be followed literally.
- The durable convention is task-scoped artifacts under
  `specs/vk/<task-id>/`, though this task's injected pipeline explicitly names
  root `SPEC.md`, `PRIOR_KNOWLEDGE.md`, and `IMPLEMENTATION_PLAN.md`; the explicit
  task instruction is authoritative for these three files.
- Constitution numbering must be rechecked against the latest base immediately
  before merge if the constitution itself changes.

### `docs/knowledge-base/codex-rollout-transfer.md` and
`docs/knowledge-base/active-mcp-refresh.md`

- Codex runs through the stdio app-server protocol and receives execution-scoped
  configuration. The actual app-server launch and thread-start boundaries are
  therefore the correct places to verify fail-loud CLI flags and emitted config
  keys.
- Avoid widening a focused executor-config correction into changes to Codex home,
  credentials, rollout persistence, or MCP configuration ownership.

## Adjacent findings

- No durable `docs/knowledge-base` topic currently records the i18n key-set
  comparison/sort-order invariant or the API envelope lesson. Those are
  candidates for stage 12 if confirmed by implementation.
- `wiki/kanban-issue-panel-sections.md` notes that i18n tests without a provider
  may return raw keys. This task instead tests locale JSON/key consistency and
  should not mistake component fallback behavior for translation coverage.
- `wiki/project-context-map.md` records the broader fail-loud principle: reject
  unknown keys at the boundary rather than accepting and discarding meaning.

## Consequences for implementation

1. Fix and test the i18n comparison algorithm itself, not merely the currently
   missing translations.
2. Extract one helper-error message mapping parallel to the poller mapping and
   assert every variant reaches `ApiResponse.message`.
3. Trace `include_apply_patch_tool` history before removal, verify the pinned
   Codex CLI's strict-config support, and pin the exact adopted launch contract
   in tests.
4. Keep all work inside the Vibe Kanban repository and use focused verification
   before the full frontend/backend gates.
