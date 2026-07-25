# Technical Plan: Incremental Bundled Pipeline Seeding

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The change is isolated to the Rust pipeline service in
`crates/services/src/services/pipelines/mod.rs`. Pipeline definitions are
ordinary files under the configured pipelines directory. No database, API, UI,
or generated type is required. Windows uses the existing `windows-sys`
dependency family for replace-existing rename semantics.

## Architecture & Approach

Add a private seed-state file containing a format version and the deterministic
list of bundled TOML filenames known at the last successful reconciliation.
Keep an explicit legacy baseline for manifest-less installations representing
the bundle immediately before `parallel-subagents.toml`.

`ensure_seeded` will compute its prior state as follows:

- no pipeline TOMLs: no prior bundled IDs, so every current bundle entry is a
  candidate;
- manifest exists: parse and validate it, then use its recorded filenames;
- non-empty without manifest: use the legacy compatibility baseline.

Reconciliation calls within the application process are serialized so rollback
cannot remove a file after a concurrent loader has committed metadata for it.

For each current bundle entry not in prior state, create it only if its target
path is absent. Existing files are never written. Track only files created by
the current call. After all candidates succeed, atomically replace the manifest
using a same-directory temporary file. On an error before commit, remove files
created by the call and the uncommitted temporary metadata file.

The current bundle list is then the authoritative manifest content. This means
future additions require only extending `BUNDLED`; absent previously recorded
files remain interpreted as user deletions.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts.md`. No HTTP contract changes.

## Research Notes

See `./research.md`. A target-specific direct `windows-sys` dependency exposes
the platform replace-existing file primitive already present transitively.

## Constitution Check

- **I Clarity over cleverness**: the compatibility baseline and state
  transitions are named and documented rather than inferred from filesystem
  accidents.
- **II Test the contract**: focused unit tests cover upgrade, deletion, edits,
  idempotence, and failed reconciliation.
- **III Small, reversible steps**: the change stays within the existing
  file-seeding helper and private directory metadata.
- **VI Don't rebuild what shipped**: existing `BUNDLED`, `ensure_seeded`, load,
  delete, and reset paths remain the integration points.
- **XIII guest-editor principle (analogous)**: existing user-editable files are
  preserved byte-for-byte, invalid metadata is not guessed through, and private
  metadata commits via temp-file rename.

No constitution deviation or open question remains.

## Risks & Dependencies

- **Legacy ambiguity**: a manifest-less directory cannot prove deletion history.
  The explicit historical baseline resolves only the one migration boundary
  needed to introduce tracking.
- **Filesystem transaction limits**: multiple file creates cannot be truly
  atomic on all filesystems. The implementation provides logical atomicity by
  committing state last and rolling back only files created by the failed call.
  A failed cleanup remains retry-safe because the manifest was not advanced and
  existing candidate files are never overwritten.
- **Concurrent calls**: load endpoints may call seeding concurrently. Creation
  is idempotent and metadata content is deterministic; temp paths should be
  unique enough to avoid one call deleting another's staging file.
- **Corrupt metadata**: fail closed and leave existing pipelines loadable.

## Verification

1. Run focused pipeline service unit tests.
2. Run `cargo fmt --check`/repository formatting.
3. Run the relevant services crate checks or tests.
4. Inspect the diff and run independent Codex review.
5. Re-run focused tests after review fixes.
