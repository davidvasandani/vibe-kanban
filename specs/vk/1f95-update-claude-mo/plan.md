# Implementation Plan: Claude Fable 5.1 and Current Claude Code

**Spec:** `./spec.md`  
**Status:** Ready for tasks

## Technical Context

The change is confined to the Rust `executors` crate in the Vibe Kanban source
repository unless deployment inspection reveals a new runtime prerequisite.
Claude Code is launched through a pinned npm wrapper string in
`crates/executors/src/executors/claude.rs`. The same module owns a static
fallback model catalog because Claude Code does not provide Vibe Kanban a
stable, complete discovery protocol.

The current pin is 2.1.200; npm's `latest` channel and its platform-native
package resolve to 2.1.268 on 2026-09-11. Fable 5.1 is identified as
`claude-fable-5-1` by Anthropic documentation and by 2.1.268's native catalog.

## Architecture & Approach

1. Extend `default_discovered_options()` with one explicit
   `("claude-fable-5-1", "Fable 5.1")` entry. The existing family predicate
   already gives IDs containing `fable` the five supported effort choices.
2. Change the standard Claude base command from 2.1.200 to 2.1.268. Keep the
   router command unchanged because it is a separate package and the request
   concerns current Claude Code dependencies.
3. Update all deliberate 2.1.200 twins in source comments and tests. Preserve
   the denied-tool constant because direct inspection of the 2.1.268 native
   binary confirms every canonical and alias mapping remains valid.
4. Add focused tests that inspect `default_discovered_options()` for the exact
   Fable 5.1 ID/label and complete effort set. Retain the exact binary-version
   assertion beside the native tool-name test.
5. Inspect `homelab/modules/vibe-kanban-rebuild.nix`. It supplies `claude-code`
   as a host package but Vibe Kanban's executor uses the npm-pinned command;
   unless inspection proves the module overrides that command or lacks Node 22,
   no deployment change is warranted.

## Data Model

See `./data-model.md`. There are no persistence or API schema changes.

## Contracts

See `./contracts/claude-executor-catalog.md` for the selector and launch
contract. No new network endpoint is introduced.

## Research Notes

See `./research.md` for registry, changelog, native-binary, alias, capability,
and deployment decisions. No new dependency is added.

## Constitution Check

- **II:** focused contract tests cover the new model, effort set, version pin,
  and native safety identifiers.
- **III/VI:** the existing catalog and launch path are extended in place.
- **IX:** exact model/tool/alias evidence comes from Anthropic and the pinned
  executing artifact. The parameter-level Bash guard remains unchanged.
- **XIV:** locked pnpm installation precedes project verification.
- **XXI:** the current model propagation path remains the single resolver.

No deviations are required.

## Risks & Dependencies

- npm can move `latest` after research. The selected 2.1.268 remains a fixed,
  reviewable pin; immediately before implementation/merge, re-query the tag and
  reconcile if it moved.
- Binary string inspection is version/platform specific. Linux x64 is the
  relevant deployed artifact, and the package's optional dependencies pin all
  native variants to the same release.
- The updated wrapper declares Node >=22. The governing Nix configuration must
  be checked for a compatible runtime before deciding the homelab module needs
  no change.
