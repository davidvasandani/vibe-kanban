# Contracts: Incremental Pipeline Seeding

## Internal service contract

`ensure_seeded(dir: &Path) -> Result<(), PipelineError>`

- On success, every current bundled filename either:
  - exists already and is unchanged;
  - was introduced after the recorded known set and has been created; or
  - was already known and remains deliberately absent.
- On success, private seed state records the complete current bundle set.
- On failure, private seed state does not claim the failed reconciliation
  succeeded.
- The function does not overwrite an existing pipeline TOML.

## External contracts

No HTTP route, response shape, generated TypeScript type, or UI contract
changes. `GET /api/pipelines` observes the behavior through its existing call
to `load_pipelines`, which invokes `ensure_seeded`.
