# Implementation Plan: Three rollout loose ends

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Frontend resources: JSON locale namespaces and Bash/jq/coreutils consistency
  checking.
- Backend: Rust 2024, Axum response envelopes, serde/ts-rs generated contracts.
- Executor: Codex app-server protocol pinned to `rust-v0.144.1`, launched through
  `CommandBuilder`.
- Scope: Vibe Kanban only; no new dependencies or persistence changes.

## Architecture & Approach

### 1. Localization data and deterministic comparison

Add `metricsDiskAlerts` adjacent to the corresponding position in each of the
six `packages/web-core/src/i18n/locales/*/common.json` resources. Preserve the
three interpolation names and the repository's `_one`/`_other` convention.

In `scripts/check-i18n.sh`, make key-list production/consumption use the same
explicit `LC_ALL=C` ordering at the `comm` boundary. Preserve failures from jq
and missing files. Exercise the full base-comparison command and independently
verify no stderr ordering diagnostics occur.

### 2. Caller-visible helper errors

Add a `start_background_helper_error_message` mapping next to the existing
poller mapping in `crates/server/src/routes/workspaces/execution.rs`. Use it at
both helper rejection sites with `error_with_data_and_message`. Add a table-driven
test that constructs the actual `ApiResponse` envelope for all variants and
asserts its message contains the corrective identifying fragment.

### 3. Fail-loud Codex configuration

Remove `include_apply_patch_tool` from the `Codex` struct and thread config map.
Regenerate `shared/types.ts` and `shared/schemas/codex.json` using
`pnpm run generate-types`.

Extend `build_command_builder` with `--strict-config` immediately after
`app-server`. Add unit coverage over the fully built initial command for both the
default and deployment-managed base command, plus a thread-config assertion that
the dead key is absent while `features.unified_exec` remains enforced.

## Data Model

No data model or migration changes.

## Contracts

- `./contracts/error-envelope.md`
- `./contracts/codex-config.md`

## Research Notes

See `./research.md`. No new dependency is planned.

## Constitution Check

- II: tests target the response envelope, built command, and full i18n gate.
- VI: helper behavior mirrors the existing poller message implementation.
- IX: dead vendor identifiers are removed and strict mode is verified against
  pinned upstream source.
- XIV: locked dependencies precede formatter/frontend verification.
- XXI: every agent-facing helper failure names the problem and correction.
- No deviation remains open. The stale generated SpecKit path was refused under
  the existing artifact-ownership contract and this task uses its own directory.

## Risks & Dependencies

- Strict mode may expose pre-existing unknown keys in user-owned Codex config;
  this is intended fail-loud behavior but must produce a useful startup error.
- Removing the generated setting is a contract change for saved executor JSON;
  serde ignores the old field on read, allowing existing saved settings to load
  while preventing future emission.
- The full i18n script clones `main` and needs network plus installed frontend
  dependencies; focused key checks should distinguish setup failures from logic.
