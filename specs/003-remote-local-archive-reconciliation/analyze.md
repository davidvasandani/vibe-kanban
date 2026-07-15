# Analysis: spec <-> plan <-> contracts <-> tasks cross-check

**Task**: `vk/f464-vk-workspace-mgm`
**Constitution checked**: `.specify/memory/constitution.md` v0.3.0

## Requirement coverage

| Req | Covered by | Status |
| --- | --- | --- |
| FR-1 detect remote archived/local active linked rows | plan Changes 1-3, selector contract, T001-T003 | OK |
| FR-2 archive through existing local update API | plan Changes 4, hook contract, T004 | OK |
| FR-3 idempotent in-flight dedupe | hook contract, plan Risks, T004/T007 | OK |
| FR-4 failure isolation | hook contract, T004/T007 | OK |
| FR-5 retry after data changes/remount | research Decisions, hook contract, T004/T007 | OK |
| FR-6 no automatic unarchive | selector contract, data model, T006 | OK |
| FR-7 ignore remote-only/unlinked local rows | selector contract, T006 | OK |
| FR-8 remote mutation unchanged | plan Approach/Constitution, T011 scope check excludes remote mutation files | OK |

## Constitution check

- **I Clarity**: Pure selector isolates the rule; hook isolates side effects. OK
- **II Test the contract**: Selector and dispatch contracts have explicit tests
  in T006/T007. OK
- **III Small/reversible**: One new module, one provider invocation, no schema or
  API change. OK
- **IV Shared-component boundaries**: No UI component change; provider owns data
  convergence. OK
- **V Remote mutations transaction/txid**: Existing remote issue archival is
  preserved as the upstream signal. OK
- **VI Don't rebuild what shipped**: Reuses current shapes, contexts, and local
  workspace update API. OK

## Gaps / risks found

- **Resolved: provider-context safety**. `ProjectProvider` is rendered under
  `WorkspaceProvider` for normal project routes, but it is also wrapped directly
  by command/dialog flows such as `WorkspaceSelectionDialog`,
  `ProjectSelectionDialog`, and related kanban dialogs. Calling the throwing
  `useWorkspaceContext()` from `ProjectProvider` would make those mounts depend
  on an ambient provider and could crash instead of simply skipping
  reconciliation when local workspace data is unavailable. `plan.md`,
  `contracts/reconciliation.md`, `research.md`, and `tasks.md` now require a
  nullable `WorkspaceContext` read or equivalent safe input path, with
  reconciliation disabled when local workspace state is absent.
- Hook testing in `@vibe/web-core` currently runs in a node Vitest environment
  without React Testing Library. The plan avoids adding a dependency by allowing
  the in-flight dispatcher to be tested as a small exported async runner/factory
  if direct hook testing is awkward.

## Ambiguities

- None remaining. The feature remains frontend-only and must not alter remote
  issue mutation transaction/txid behaviour.

**Verdict**: consistent after the provider-context safety planning update; cleared
for implementation.
