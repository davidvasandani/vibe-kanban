# Contracts: Reliable Parallel Sub-Agent Pipeline

## Bundled prompt contract

The four existing stage IDs and defaults remain stable. When enabled:

- `fanout` requests concurrent Claude/Codex/Grok analysis with unchanged
  initial task input and workspace-reading tools under non-mutating policy;
- `analyze` compares only actual labeled provider outputs and identifies absent
  providers;
- `iterate` performs at most `N` completed rounds with fresh concurrent
  children and original-plus-synthesis context;
- `code-review` remains optional and unchanged.

## Seed reconciliation contract

`ensure_seeded(dir: &Path) -> Result<(), PipelineError>` gains one compatible
postcondition: a present exact previous bundled parallel default becomes the
current default. Existing guarantees remain:

- customized existing files are not overwritten;
- known deleted defaults are not recreated;
- reconciliation is serialized;
- writes fail without leaving a partially truncated destination.

## External contracts

No route, response, shared type, database, UI, or executor protocol contract
changes.
