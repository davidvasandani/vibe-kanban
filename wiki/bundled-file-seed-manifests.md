# Incrementally seeding user-editable bundled files

Bundled defaults that are copied into a user-editable directory need more state
than “the directory contains a file.” That directory-level check handles first
run, but it cannot deliver a default added by a later release.

The opposite shortcut — create every missing bundled filename — is also unsafe.
Once defaults are user-editable, an absent old filename may be an intentional
deletion, while an absent new filename has never been offered to the user.

## Record what the installation has seen

Pipeline seeding stores a private manifest beside the user-facing TOMLs. The
manifest records the bundled filenames known at the last successful
reconciliation:

- a current bundled filename absent from the recorded set is newly introduced
  and may be created if its target does not already exist;
- a filename already recorded but now absent is treated as a user deletion and
  remains absent;
- an existing target is never overwritten, preserving local edits;
- after successful reconciliation, the manifest advances to the complete
  current bundle set.

The manifest does not use the user-content extension, so ordinary discovery
ignores it.

## Bootstrap is migration data

A directory created before manifests existed is inherently ambiguous. Inferring
the known set from files currently present repeats the original bug: a newly
bundled missing file would look already handled. Treat the bundle set from the
release immediately before manifest support as explicit migration data.

For pipelines that baseline is `basic.toml`, `wikillm.toml`, and
`speckit.toml`. Therefore `parallel-subagents.toml` is new to a manifest-less
existing install, while a missing baseline file remains a deletion.

This baseline is a one-time compatibility boundary. Once a manifest exists,
future additions require only a new entry in the canonical bundled catalog.

## Commit metadata last

Seed reconciliation is a small filesystem transaction:

1. Serialize concurrent reconciliation calls within the process.
2. Validate existing metadata before writing anything, including when no
   user-facing files remain.
3. Create candidate defaults with exclusive-create semantics.
4. Track only files created by this call.
5. Atomically replace the manifest after every candidate succeeds.
6. If a candidate or manifest write fails, remove files created by the call and
   leave the previous manifest authoritative.

The manifest is written to a same-directory temporary file, flushed, then
renamed. Unix rename replaces the destination atomically; Windows needs
`MoveFileExW` with replace-existing and write-through flags because standard
Rust rename rejects an existing destination there.

Malformed metadata fails closed. Reconstructing it from disk cannot distinguish
a deletion from an unseen bundle and may undo user intent. Callers may still
load the valid files already present while logging the reconciliation failure.

## Testing the contract

At minimum, cover:

- a manifest-less legacy install receives the first incremental default;
- a recorded then deleted default stays deleted;
- edited bytes are unchanged;
- reconciliation is idempotent;
- malformed metadata causes no writes;
- a later candidate failure rolls back earlier files and does not commit the
  manifest.

## Contributed by

- vk/6e84-seed-newly-bundl
