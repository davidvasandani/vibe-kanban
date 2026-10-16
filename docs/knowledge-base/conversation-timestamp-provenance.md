# Conversation timestamp provenance

Tags: `vk/a22f-time-stamp-chat`, `vk/77a2-session-step-tim`

## Preserve event time through the display projection

Workspace chat consumes normalized executor entries through several projection
layers: process-scoped patch keys, semantic turn derivation, aggregation, row
modelling, and virtualization. Timestamp metadata belongs on the existing entry
throughout those layers. A standalone timestamp row changes row counts and keys,
which destabilises history anchors and navigation.

`NormalizedEntry.timestamp` is authoritative when valid. Fresh execution stores
use `MsgStore::live()` to fill missing/invalid times at first normalized-patch
publication. This is server observation time, not necessarily vendor occurrence
time: delivery can be delayed. Replacements retain that time unless a valid
source timestamp is supplied. The existing completed-execution normalized sidecar
persists the enriched values.

`MsgStore::new()` deliberately does not stamp: historical normalization and
worker reattachment replay old output. Enabling timestamps at generic normalizer
startup would misdate replay. Fresh child, worker and detached-file tracking opt
in explicitly; adoption/reattachment does not. If a restart or failed sidecar
write leaves only raw logs, unknown step times remain unknown.

The old frontend `processCreatedAt` fallback made every untimed step look like
it occurred at execution start (the reported frozen timestamp). Never use that
fallback for step rows, and never use browser render/receipt time.

Timestamp identity follows JSON Patch array semantics. Add shifts later indices
right, remove shifts them left. AmpResume clears history with repeated removals
at index zero before resetting its provider; deleting only the literal map key
leaks old times into new steps. The ordered metadata map is bounded at 100,000
entries; new unknown timestamps remain unset if full.

Synthetic prompt and script rows are created after raw history is projected.
They must copy the same process creation time explicitly, both into the
normalised entry and its keyed-patch parent metadata.

## Format only at the shared row boundary

The `DisplayEntry` boundary is common to atomic messages/actions and aggregated
tool, diff, and thinking rows across local and remote web clients. Formatting
there avoids widening every specialised UI component and preserves shared
behaviour.

Use these rules:

1. Prefer a valid entry timestamp.
2. Omit step time when entry time is absent or malformed; do not substitute
   parent process time.
3. For an aggregate, select the greatest valid member time.
4. Show time only for the current local day; add a short local date otherwise.
5. Preserve the source value in semantic `<time dateTime>` output and expose a
   full local date/time as hover and accessible text.
6. Omit timestamps for transient or deliberately hidden entries. In particular,
   mirror renderer suppression for AskUserQuestion calls and empty plan
   placeholders, or the wrapper creates orphan timestamp-only rows.

## Regression coverage

Test timestamp selection separately from presentation so locale behaviour is
deterministic with an injected `now` and locale. Cover absent and malformed
values, rejection of process fallback, same-day versus cross-day labels, latest-member group
selection, semantic markup, and every renderer path that intentionally returns
no visible content.

Backend regressions cover distinct live steps, replacement/source priority, array
insertion/removal and Amp reset, bounded metadata, replay exclusion, and
completed-sidecar preservation of live time.
