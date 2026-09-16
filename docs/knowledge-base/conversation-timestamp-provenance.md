# Conversation timestamp provenance

Tags: `vk/a22f-time-stamp-chat`

## Preserve event time through the display projection

Workspace chat consumes normalized executor entries through several projection
layers: process-scoped patch keys, semantic turn derivation, aggregation, row
modelling, and virtualization. Timestamp metadata belongs on the existing entry
throughout those layers. A standalone timestamp row changes row counts and keys,
which destabilises history anchors and navigation.

`NormalizedEntry.timestamp` is authoritative when it is valid. Most executor
normalisers currently omit it, so every keyed patch also carries its owning
execution process's `created_at` as a server-backed parent fallback. Never use
render or receipt time: historical replay would then look like current activity.

Synthetic prompt and script rows are created after raw history is projected.
They must copy the same process creation time explicitly, both into the
normalised entry and its keyed-patch fallback metadata.

## Format only at the shared row boundary

The `DisplayEntry` boundary is common to atomic messages/actions and aggregated
tool, diff, and thinking rows across local and remote web clients. Formatting
there avoids widening every specialised UI component and preserves shared
behaviour.

Use these rules:

1. Prefer a valid entry timestamp.
2. Fall back to a valid parent process time when entry time is absent or
   malformed.
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
values, process fallback, same-day versus cross-day labels, latest-member group
selection, semantic markup, and every renderer path that intentionally returns
no visible content.
