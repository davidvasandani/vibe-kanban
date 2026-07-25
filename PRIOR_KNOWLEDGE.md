# Prior Knowledge: Claude Opus 5 Model Support

The project knowledge base is populated, but it does not yet contain a page
about maintaining hard-coded executor model catalogs. The closest relevant
pages are:

- `docs/knowledge-base/grok-executor-integration.md`: executor changes can span
  Rust implementation, generated schemas/types, frontend presentation, and
  tests. Its broad cross-product checklist is useful, although this task extends
  existing executors rather than introducing a new one.
- `docs/knowledge-base/claude-log-normalization.md`: the Claude executor and its
  tests are concentrated in `crates/executors/src/executors/claude.rs`.
  This task should preserve those log-processing contracts and limit changes to
  discovery/model metadata unless model context behavior requires otherwise.
- `wiki/managed-cli-tool-catalog.md`: generated artifacts should be refreshed
  from their Rust sources with repository generation commands and must not be
  edited directly. Although this page concerns managed CLIs, the same
  repository invariant applies to executor JSON schemas and shared TypeScript
  types.

## Planning constraints distilled from the knowledge base

1. Trace each changed Rust model catalog into generated schema/type artifacts
   before deciding the final file set.
2. Prefer focused executor tests and generation checks over frontend
   special-casing when the existing UI consumes discovery metadata generically.
3. Keep the task additive: preserve existing model entries and executor
   behavior.
4. If implementation reveals a reusable cross-executor model-catalog update
   procedure, record it as a new knowledge-base topic after the change ships.
