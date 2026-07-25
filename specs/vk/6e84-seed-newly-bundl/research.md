# Research: Incremental Bundled Pipeline Seeding

## Historical migration boundary

Repository history shows `basic.toml`, `wikillm.toml`, and `speckit.toml`
shipped before `parallel-subagents.toml`. The preexisting deletion contract
means filenames missing from those first three cannot safely be recreated.

Decision: encode those three names as the one-time legacy baseline for a
non-empty directory without seed metadata.

Rejected alternative: initialize the manifest from files currently on disk.
That would classify the newly bundled `parallel-subagents.toml` as already known
when absent, reproducing the bug.

Rejected alternative: seed every missing bundle entry once. That resurrects
historical user deletions.

## State representation

Decision: use a private JSON manifest with a format version and bundled
filenames. JSON serialization is already available through `serde_json` in the
services crate, is unambiguous, and avoids a `*.toml` extension that pipeline
loaders would discover.

Rejected alternative: one integer release version. A set of filenames directly
captures the semantic question ("was this bundled ID already known?") and avoids
coupling application versions to bundle changes.

## Failure model

Decision: write candidate files without overwrite, track files created by this
call, commit deterministic metadata last through a same-directory temp file and
rename, and best-effort roll back created candidates on failure.

True multi-file atomicity is unavailable through portable standard filesystem
operations. Committing the manifest last is the essential invariant: any
failure remains retryable and cannot permanently mark an unwritten candidate as
seeded.

## Invalid metadata

Decision: fail closed. Reconstructing from disk cannot distinguish deletion
from absence and could violate user intent. Existing load functions already log
seeding failures and continue loading valid files, so this does not make the
pipeline endpoint wholly unavailable.

## Dependency review

The services crate already uses `serde` and `serde_json`. A target-specific
direct dependency on the already-transitive `windows-sys` 0.61 family is
required because Rust's standard `rename` cannot atomically replace an existing
destination on Windows. `MoveFileExW` with replace-existing and write-through
flags supplies equivalent manifest commit behavior there.
