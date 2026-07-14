# Implementation Plan: Shared MCP Servers

This plan is based on `SPEC.md` and the workspace-root `PRIOR_KNOWLEDGE.md`. SpecKit stages may refine file-level details, but implementation must preserve native agent configuration formats and existing test/OAuth semantics.

1. **Establish the shared domain model.** Add Rust DTOs for a logical MCP server, its target agents/profiles, source/native representations, conflicts, and per-target mutation outcomes. Register exported DTOs with the existing ts-rs generator.
2. **Build deterministic reconciliation.** Implement pure helpers that read each MCP-capable executor's native server map, normalize comparable known transports, consolidate equivalent same-name entries, retain single-agent/custom entries, and emit explicit conflicts for incompatible same-name definitions. Add unit tests first for identical, adapted, custom, and conflicting inputs.
3. **Add shared read/write routes.** Add an authenticated shared MCP configuration endpoint that enumerates MCP-capable executors and config paths, returns reconciled state, validates assignments, and fans a desired shared state out through each executor's existing `McpConfig` writer. Preserve unrelated native settings. Return per-executor outcomes and do not claim full success on partial failure.
4. **Keep assignment-aware operations.** Extend or wrap connectivity test and OAuth entry points so requests identify the target executor/profile. Continue probing saved native entries and refreshing OAuth-modified disk state.
5. **Regenerate client types and API bindings.** Register new Rust types, run the project type generator, and update local/relay machine-client methods to call the shared endpoints without hand-editing generated type files.
6. **Redesign MCP settings state.** Replace the primary executor selector flow with a unified logical-server collection. Load once, track a stable saved snapshot, surface reconciliation conflicts and partial errors, and save definitions plus assignments as one user action.
7. **Add profile assignment controls.** Update the server dialog/card UI with an MCP-capable multi-select, require at least one assignment, show assigned profile names/count, and preserve the raw JSON escape hatch for definitions that cannot be represented by the standard form.
8. **Make test and OAuth results per assignment.** Allow testing a server's assigned native representations and render each result beside its profile. Run OAuth Connect against one chosen assignment and merge the refreshed native credentials without erasing unrelated unsaved edits.
9. **Update language strings and documentation.** Revise English MCP settings copy from per-agent configuration to shared servers/assignments, keep locale fallbacks structurally safe, and update user-facing MCP settings documentation if behavior or screenshots materially changed.
10. **Verify focused behavior.** Run backend unit tests for reconciliation/write planning, frontend codec/state tests, generated-type checks, formatting, and the narrowest practical workspace checks. Resolve failures attributable to the change.
11. **Independent review.** Run the repository's Codex review workflow against the final diff, address confirmed significant findings, and repeat review/checks until none remain.
12. **Capture reusable knowledge.** Add or update an MCP configuration-sharing topic in the project knowledge base, tag it with `a898-allow-mcp-server`, refresh its index, and commit the knowledge-base update as required by the pipeline.

## Dependency and Parallelism Notes

- Steps 2–4 depend on the model in step 1.
- Step 6 depends on the shared read model and generated TypeScript types.
- UI assignment controls and copy can proceed together after the frontend state shape is fixed.
- Tests should land with the helper or component they cover, rather than as a final bulk addition.
- Multi-file writes are initially best-effort with explicit per-target results unless SpecKit clarification requires rollback semantics; no misleading all-or-nothing response is permitted.
