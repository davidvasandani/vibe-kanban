# Tasks: Incremental Bundled Pipeline Seeding

**Feature**: `specs/vk/6e84-seed-newly-bundl/`
**Task**: `6e84-seed-newly-bundl`

## Layer 1 — Seed-state contract

- [x] T001 Add manifest constants, the historical compatibility baseline, and
  validated serialization/deserialization helpers in
  `crates/services/src/services/pipelines/mod.rs`.
- [x] T002 Implement atomic manifest replacement and cleanup helpers in the same
  module. Depends on T001.

## Layer 2 — Reconciliation

- [x] T003 Refactor `ensure_seeded` to derive the effective known bundle set,
  create only newly introduced missing files, preserve existing/deleted known
  files, commit metadata last, and roll back created candidates on failure.
  Depends on T001 and T002.
- [x] T004 Confirm explicit reset and delete paths remain compatible; make only
  the minimum metadata-consistency adjustment if tests demonstrate one is
  needed. Depends on T003.

## Layer 3 — Contract tests

- [x] T005 Add focused unit tests for manifest-less existing-install upgrade,
  deleted-file preservation, local-edit preservation, and idempotence in
  `crates/services/src/services/pipelines/mod.rs`. Depends on T003.
- [x] T006 Add tests for fresh/TOML-empty seeding, invalid metadata, and failed
  reconciliation bookkeeping/cleanup. Depends on T003.

## Layer 4 — Verification

- [x] T007 Run focused pipeline service tests. Depends on T004, T005, T006.
- [x] T008 Run repository formatting and the relevant services crate check/test,
  then inspect the diff for unrelated changes. Depends on T007.

## Layer 5 — Review and knowledge

- [x] T009 Run independent Codex diff review, address confirmed significant
  findings, rerun relevant validation, and repeat until clean. Depends on T008.
- [x] T010 Record the reusable versioned bundled-file seeding pattern in the
  project knowledge base, tag it with `vk/6e84-seed-newly-bundl`, refresh the
  index, and commit the completed change. Depends on T009.

## Parallel execution notes

The implementation and tests share one Rust module and should remain sequential
to avoid overlapping edits. Once implementation is stable, focused tests and
format/check commands are read-only but are ordered here so failures are easier
to attribute.
