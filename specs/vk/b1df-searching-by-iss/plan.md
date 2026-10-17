# Implementation Plan: Search by Issue ID

**Spec**: `./spec.md`
**Status**: Implemented

## Technical Context

The remote metadata API is Rust/Axum with PostgreSQL/SQLx. Its existing
`GET /v1/global-search` handler and static SQL live in
`crates/remote/src/routes/global_search.rs`. The shared React/TypeScript dialog
and aggregation logic live in `packages/web-core`, serving both local and remote
frontends. The existing isolated PostgreSQL harness at
`scripts/test-global-search-postgres.py` executes the production SQL directly.

Issue records already expose an internal UUID, `simple_id`, title, and project
relationship. No schema migration, generated shared type, new endpoint, or new
dependency is required.

## Architecture & Approach

1. Extend the remote search SQL's `matches` CTE with an `issue` branch joining
   `issues` to the already membership-scoped project/organization set. Match
   `simple_id` with the existing case-insensitive literal substring convention.
2. Represent an issue result with the existing envelope: issue UUID in `id` and
   `issue_id`, owning UUID in `project_id`, title in `title`, and a context that
   begins with `simple_id` followed by organization/project names.
3. Let the existing `ranked` CTE partition issues as another category, retaining
   the 21-row sentinel used to detect a 20-result category limit.
4. Extend `GlobalSearchResult.kind` and `GlobalSearchDialog` with `issue` and an
   `Issues` group. Reuse `searchResultHref` and the dialog's existing
   organization-selection callback before navigation.
5. Extend the PostgreSQL harness with the minimal `issues` table and fixtures to
   prove case-insensitive ID matching, membership isolation, literal matching,
   and category bounds. Add Vitest coverage for issue routing and dialog display
   / selection coordination.

## Data Model

See `./data-model.md`. No persisted data model changes are required.

## Contracts

See `./contracts/global-search.md`. The existing response is extended
compatibly with one new result-kind discriminator.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: focused PostgreSQL and rendered-dialog tests verify the contract.
- Principles III and VI: the change extends the current endpoint, result
  envelope, dialog, and route builder with no parallel search mechanism.
- Principle IV: shared behavior remains in `packages/web-core`, covering both
  frontends.
- Principle VII: the UI presents `simple_id`; routing retains the issue UUID.
- Principle XXXV: each result carries its matching organization/project identity
  and membership scope; no identity is projected from a sibling entity.
- Constraints: no dependency or generated-file changes are planned, and
  repository formatting will run before completion.

No constitution deviations or open questions remain.

## Risks & Dependencies

- The direct SQL harness schema must track every table/column referenced by the
  production query or it will fail before assertions run.
- Adding issue rows changes total response size but not per-category bounds; tests
  should assert category counts rather than a legacy total.
- UI result ordering is by kind string, so the new category may alter inter-group
  ordering. The dialog explicitly controls presentation order to keep it stable.
- PostgreSQL binaries may require a Nix development environment; if unavailable,
  backend compilation plus review of the production-SQL harness must be reported
  distinctly rather than silently skipped.
