# Tasks: MCP Identifier and Display-Label Separation

**Plan**: `./plan.md`

Tasks are dependency ordered. Tasks marked **[P]** touch independent files and
may run together within their phase.

## Phase 1: Backend contract and metadata store

- [x] T001 Add `display_name` to shared server/input/conflict Rust DTOs with
      backward-compatible serde defaults in
      `crates/executors/src/shared_mcp_config.rs`.
- [x] T002 Add a testable, versioned, atomic label sidecar store and label
      normalization in `crates/executors/src/shared_mcp_config.rs` (depends on
      T001).
- [x] T003 Decorate native read results and conflicts by identifier from the
      label store without changing fingerprints/materialization in
      `crates/executors/src/shared_mcp_config.rs` (depends on T002).
- [x] T004 Validate identifiers and duplicates before writes; normalize labels
      independently so metadata failures do not block valid native writes;
      converge the sidecar only after a relevant native success and report
      metadata failures truthfully in
      `crates/executors/src/shared_mcp_config.rs` and
      `crates/server/src/routes/config.rs` (depends on T002–T003).
- [x] T005 Add/extend Rust tests for suggestion parity, label persistence,
      malformed/missing sidecars, collision rejection, read decoration,
      partial-write ordering, explicit rename planning, and absence of label
      fields from native definitions in
      `crates/executors/src/shared_mcp_config.rs` and
      `crates/server/src/routes/config.rs` (depends on T001–T004).
- [x] T006 Regenerate shared TypeScript declarations through
      `crates/server/src/bin/generate_types.rs` into `shared/types.ts` (depends
      on T001).

## Phase 2: Frontend identity model

- [x] T007 Add shared identifier validation/suggestion and presentation helpers
      with table-driven parity tests in
      `packages/web-core/src/shared/lib/mcpServerIdentifier.ts` and
      `packages/web-core/src/shared/lib/mcpServerIdentifier.test.ts` (depends on
      T001 contract only).
- [x] T008 Extend draft conversion, snapshots, inputs, conflict promotion, and
      OAuth refresh merge with display labels in
      `packages/web-core/src/shared/lib/sharedMcpSettingsState.ts` and
      `packages/web-core/src/shared/lib/sharedMcpSettingsState.test.ts` (depends
      on T006).
- [x] T009 Update `McpServerDialog` to edit Identifier and Display name
      separately, seed explicit repair for unsafe legacy names, warn on rename,
      and use the shared helper in
      `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.tsx`
      (depends on T007–T008).
- [x] T010 Update catalog Add, original-name removal, cards, JSON mode, and all
      test/auth/refresh/debug state lookups in
      `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
      so presentation uses labels and operations use identifiers (depends on
      T008–T009).
- [x] T011 [P] Add/update MCP label, identifier, collision, and rename-warning
      strings in `packages/web-core/src/i18n/locales/*/settings.json` (depends on
      T009 field semantics).
- [x] T012 Add focused dialog/settings tests for catalog labels, legacy repair,
      collision behavior, rendered secondary identifier, and identifier-keyed
      actions in
      `packages/web-core/src/shared/dialogs/settings/settings/McpServerDialog.test.tsx`
      and existing MCP settings test files as appropriate (depends on T009–T011).

## Phase 3: Verification and documentation

- [x] T013 Run focused executors/server Rust tests and web-core Vitest suites;
      fix failures in files already listed above (depends on T005, T012).
- [x] T014 Run `pnpm run generate-types:check`, `pnpm run check`, applicable
      lint, and `pnpm run format`; fix only task-scoped findings (depends on
      T013).
- [x] T015 Exercise add/save/reload for “Atlassian Rovo,” inspect a native
      config fixture for the safe key/no label field, and verify test/auth
      targets use that identifier (depends on T014).
- [x] T016 Run independent Codex diff review, address confirmed findings, and
      repeat focused verification/review until no significant findings remain
      (depends on T015).
- [x] T017 Update `docs/knowledge-base/shared-mcp-configuration.md` and
      `docs/knowledge-base/INDEX.md` with the shipped identity/display rule and
      task id `vk/a5f8-concat-repeating`, then commit the knowledge-base update
      (depends on T016).

## Dependency Graph

```text
T001 -> T002 -> T003 -> T004 -> T005 -> T013
  |                                 \
  +-> T006 -> T008 -> T009 -> T010 -> T012 -> T013
              ^       ^        ^
T007 ----------+-------+        +-- T011 [P]
T013 -> T014 -> T015 -> T016 -> T017
```
