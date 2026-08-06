# Technical Plan: Reliable Parallel Sub-Agent Pipeline

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The user-facing behavior is defined by
`assets/pipelines/parallel-subagents.toml` and embedded by the pipeline service.
Already seeded copies live in user-editable storage and are reconciled by
`ensure_seeded` in `crates/services/src/services/pipelines/mod.rs`. No database,
HTTP, generated type, or frontend change is required.

## Architecture & Approach

Rewrite the bundled prompt fragments as an explicit orchestration contract:

- launch all three named providers concurrently;
- deliver the unchanged original task as initial input;
- retain workspace-reading capability and request non-mutating analysis through
  prompt plus read-only sandbox/permission policy;
- collect labeled final outputs, isolate failures, and never fabricate missing
  provider responses;
- start fresh concurrent children for each later completed round, with original
  prompt and previous synthesis clearly separated.

Add a single historical-content constant for the previously shipped parallel
pipeline. During existing `ensure_seeded` reconciliation, check a present,
already-known `parallel-subagents.toml`. If its bytes equal that historical
constant, atomically replace it with the current embedded default using the
module's existing same-directory temp-file and replace primitive. If it is
absent or differs by any byte, preserve it. The migration is naturally
idempotent because the new content no longer matches the old constant.

Strengthen the existing bundled-pipeline test to assert semantic phrases for
tool retention, prompt delivery, concurrency, bounded completed rounds, fresh
children, failure isolation, and non-substitution. Add seeding tests for exact
legacy upgrade, one-byte customization preservation, deletion preservation, and
idempotence.

## Supporting Artifacts

- Research: `./research.md`
- Data model/state transitions: `./data-model.md`
- Contracts: `./contracts.md`

## Constitution Check

- Principle II: focused tests pin the executable prompt and migration contract.
- Principle III: the change is confined to one bundled asset and existing seed
  reconciliation.
- Principle VI: existing prompt composition, bundle embedding, locking, and
  atomic replacement machinery are reused.
- Principle IX: the prompt uses supported agent/CLI abstractions without adding
  agent-specific backend protocol machinery.
- Principle XVIII: refresh occurs only for exact known historical bytes;
  customized, absent, and ambiguous files are preserved.

No constitution deviation remains.

## Risks

- Natural-language execution can still vary by orchestrator. Explicit negative
  and positive requirements plus regression assertions reduce ambiguity without
  creating a new runtime.
- External providers may remain unavailable. Failure isolation improves the
  partial result but cannot manufacture provider access.
- A crash during replacement must not truncate the pipeline. Reuse the
  same-directory write/sync/replace pattern already used by seed metadata.

## Verification

Run repository formatting, focused `services` pipeline tests, and the relevant
crate check. Inspect the final diff, run independent Codex review, address
confirmed findings, and rerun focused verification.
