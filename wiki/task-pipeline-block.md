# The task pipeline block: compose/parse round-trip and its editing rules

Per-task pipelines live as a generated `## Pipeline` markdown block inside
`issues.description`, bounded by `<!-- vk:pipeline:start/end -->` — there is
deliberately **no structured copy** of the selection anywhere else (one
source of truth; hand-edits, integrations, and Jira sync can all rewrite the
description). All logic is pure functions in
`packages/web-core/src/shared/lib/pipeline/taskPipeline.ts`, used by
`PipelineSection` (create + edit modes of the kanban issue panel) and by the
stage-progress renderer (`parsePipelineStages`, `VK-PIPELINE-STAGE: N`
markers).

## The round-trip contract

- `composePipelineBlock(pipelines, enabledIds, …)` generates the block;
  `parsePipelineSelection(block, catalog)` is its **best-effort inverse**
  (heading names → pipeline ids, numbered lines whose remainder exactly
  equals a `prompt_fragment` → stage ids). A block composed by the current
  composer round-trips byte-identically, which is what keeps the edit-mode
  "Update Issue" button clean on open.
- Recompose is non-destructive **only for unrecognized lines**
  (`extractManualLines`): lines that don't match any known fragment survive
  as manual text. The flip side is the key gotcha below.

## Gotcha: under-recognizing the selection is destructive

`extractManualLines` drops any numbered line whose fragment is known to the
FULL catalog but not enabled under the currently *selected* pipelines (so
unticking a stage removes its line, by design). Consequence: if seeding
fails to select the right pipeline, its stage lines are "known but
unselected" → silently dropped on recompose → an apply persists the loss.
Every parse improvement in this feature closed a variant of that hole:

- Duplicate display names (names are NOT unique across pipeline TOML
  files): candidates are scored by how many of their stages appear among
  the block's numbered lines, ties → catalog order, each id assigned once.
  Never key pipeline identity by name alone.
- Names may contain `" + "` (the heading joiner): segment the heading by
  greedy longest-match against the catalog (`segmentHeadingNames`), never
  `split(' + ')`.

## Gotcha: legacy/undelimited blocks and destructive regexes

`extractPipelineBlock` / `stripPipelineBlock` fall back to a line-anchored
heading match when the delimiters are absent (legacy tasks, hand-edits past
the delimiters), treating heading→end-of-text as the block — the same
assumption `parsePipelineStages` always made. Because `stripPipelineBlock`
**deletes** from the match onward, its regex must match only the exact
generated forms: `/^## Pipeline(?::.*)?$/m`. A `\b`-style match also hits
prose headings like `## Pipeline risks` and deletes user content. Rule:
regexes that gate destructive paths are strict; regexes that gate
display-only paths may be loose.

## Component rules (`PipelineSection`)

- The section is **uncontrolled** and seeded exactly once after the
  pipelines list loads (`initialBlock` prop parses the stored block; the
  block itself seeds the textarea so manual lines survive). Callers reseed
  by remounting via `key` — create mode keys on the composer + reset
  counter, edit mode keys on the issue id.
- **Don't reseed ticks from a selection-watching effect.** An effect keyed
  on `selectedIds` can't tell a user toggle from programmatic seeding and
  clobbers parsed ticks with the `default_enabled` union (and with a warm
  React Query cache, seed + mount runs share one effect flush, so
  "skip-once ref" workarounds mis-fire). State adjustments belong in the
  event handler (`togglePipeline`), where the cause is known.
- Pipeline toggles adjust ticks **incrementally**: adding a pipeline
  enables only that pipeline's default stages on top of current ticks;
  removing one drops only stages no longer declared by any selected
  pipeline. A blanket reset to the default union silently discards the
  operator's customization (Codex round-3 finding).

## Edit-mode apply (container)

`KanbanIssuePanelContainer` applies the card via the existing `updateIssue`
mutation: cancel the pending debounced description save first (a stale
debounce would resurrect the old block), build on `latestDescriptionRef`
(not a snapshot) so concurrent prose edits survive, skip while attachment
uploads are pending (temporary local sources must not be persisted), and
mirror into local edit state + the ref. Dirty = composed block vs
`extractPipelineBlock(live description)`, both trimmed; the Update Issue
button is disabled-when-clean rather than hidden.

## Contributed by
- vk/77eb-vk-pipeline
