# Implementation Plan: Task-Scoped Pipeline Design Records

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The affected behavior is defined by bundled TOML assets loaded at compile time
by the Rust pipeline service. `assets/pipelines/wikillm.toml` and
`assets/pipelines/speckit.toml` supply verbatim stage prompt fragments;
`crates/services/src/services/pipelines/mod.rs` includes, seeds, parses, and
tests them. No API, database, frontend, generated type, or deployment change is
needed.

## Architecture & Approach

1. Edit only the three WikiLLM prompts that currently select shared artifact
   filenames. Preserve stage IDs and metadata. Each prompt names its canonical
   `specs/vk/<task-id>/...` path and defines `<task-id>` as the identifier from
   the current task or task branch.
2. Extend SpecKit's constitution prompt to make draft-time numbering
   provisional. Extend the WikiLLM and SpecKit merge prompts with the operational
   collision protocol: immediately before merge, inspect the latest actual
   base-branch tip, choose the next free number, renumber the unmerged addition,
   and update its internal references rather than moving a merged principle.
3. Add focused assertions in
   `crates/services/src/services/pipelines/mod.rs` that load bundled WikiLLM and
   SpecKit definitions and pin both the draft-time and merge-time semantic
   clauses. Retain the exact Basic pipeline test because Basic is out of scope.
4. Do not change bundle seeding semantics. New/reset bundled defaults receive
   the text; previously customized machine-local files remain user-owned.

## Data Model

No data-model change. Pipeline file schema and in-memory `PipelineStep` remain
unchanged.

## Contracts

See `./contracts/prompt-contract.md` for the exact behavioral contract.

## Research Notes

See `./research.md`. No dependency is added.

## Constitution Check

- Principle I: prompts and tests use direct, readable language.
- Principle II: loader tests verify the executable prompt contracts.
- Principle III: the change is limited to bundled text and focused assertions.
- Principle VI: existing bundle loading/reset machinery is reused.
- Constraint compliance: no dependency, generated-file, UI, or deployment
  changes; formatting will run before completion.

No constitution deviation or open question remains.

## Risks & Dependencies

- Agents could create a literal `<task-id>` directory. Explicit derivation text
  and tests mitigate this.
- Existing installed defaults do not update automatically. This preserves
  user-owned customizations and relies on the established reset workflow.
- Exact-string tests can become brittle. Assertions pin only the required
  semantic clauses for WikiLLM/SpecKit while leaving the existing Basic
  verbatim compatibility test intact.
