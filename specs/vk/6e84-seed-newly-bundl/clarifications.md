# Clarifications: Incremental Bundled Pipeline Seeding

## Decisions

### Compatibility baseline

A manifest-less non-empty installation is treated as having reconciled the
three bundled filenames that predate incremental seeding:

- `basic.toml`
- `wikillm.toml`
- `speckit.toml`

This is the only inference that simultaneously makes
`parallel-subagents.toml` appear after its release and preserves intentional
deletions of older defaults. The baseline is explicit migration data, not
derived from whichever files happen to exist.

### Invalid seed state

An invalid or unreadable seed-state file causes reconciliation to return an
error. It is not reconstructed heuristically, because guessing could resurrect
a deliberately deleted bundled file. Pipeline loading continues using the
existing files and logs the seeding error, matching current load behavior.

### Reset behavior

Explicit reset operations retain their current overwrite semantics. Successful
automatic reconciliation is responsible for seed metadata; a reset need not
erase deletion history because restoring a file already makes it present, and
the next reconciliation can safely bring metadata current.

## Remaining open questions

None.
