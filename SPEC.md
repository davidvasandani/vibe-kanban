# Spec: Edit-mode Pipeline editing with "Update Issue" button

Task: `vk/77eb-vk-pipeline` — "Pipelines are available when creating an issue but
they should also be available to add after the issue is created with an
'Update Issue' button."

## Background

The per-task **Pipeline** control (`PipelineSection`) lets an operator pick one
or more file-based pipelines, tick stages, and edit the composed
`## Pipeline` markdown block. The block is appended to the issue description
(delimited by `<!-- vk:pipeline:start/end -->`) when the issue is **created**
(`KanbanIssuePanelContainer.handleSubmit`, create branch). The render slot in
`KanbanIssuePanel` is gated `isCreateMode && renderPipeline`.

Today there is **no way to add or change a pipeline after creation** short of
hand-editing the raw description markdown. Groundwork already exists:
`extractPipelineBlock()` in `packages/web-core/src/shared/lib/pipeline/taskPipeline.ts`
was written "for seeding the edit-mode `PipelineSection`" but is currently
unused.

## Goal

In the issue detail panel (edit mode), show the same Pipeline control, seeded
from the issue's existing pipeline block (if any), with an **"Update Issue"**
button that persists the recomposed block into the issue description.

## Non-goals

- No backend/Rust changes: the pipeline block continues to live inside
  `issues.description`; persistence goes through the existing `updateIssue`
  mutation (ElectricSQL optimistic write).
- No change to how executions consume the block (`parsePipelineStages`,
  VK-PIPELINE-STAGE progress markers) — the block format is unchanged.
- No auto-save of pipeline edits in edit mode; changes apply only on the
  explicit "Update Issue" click (that is the requested UX).

## Design

### 1. Parse an existing block back into a selection (`taskPipeline.ts`)

New exported helper:

```ts
export function parsePipelineSelection(
  block: string,
  pipelines: readonly Pipeline[]
): { pipelineIds: string[]; enabledIds: string[] }
```

Best-effort inverse of `composePipelineBlock`:

- **pipelineIds**: from the `## Pipeline: <Name> + <Name>` heading — split the
  remainder on `" + "` and match each part against `pipelines[].name`
  (first match wins); unmatched names are dropped. Bare `## Pipeline` (or a
  missing heading) → `[]`.
- **enabledIds**: each numbered line `N. <rest>` whose `<rest>` exactly equals
  some stage's `prompt_fragment` (across ALL provided pipelines) maps to that
  stage id, deduped, in block order.
- Manual lines are NOT returned — they are preserved by the existing
  non-destructive recompose (`extractManualLines` via
  `composePipelineBlock({previousBlock})`) when the seeded block is used as
  the section's initial text.

### 2. `PipelineSection` gains seeding + edit-mode props

New optional props:

- `initialBlock?: string` — when non-empty, the once-only seed effect (which
  today defaults the picker to `basic`) instead calls
  `parsePipelineSelection(initialBlock, pipelines)` and seeds
  `selectedIds`, `enabledIds`, and the composed `text` from the block. A ref
  flag suppresses the immediately following "reseed ticks on selection
  change" effect once, so the parsed tick state isn't clobbered by the
  `default_enabled` union.
- `seedDefaultPipeline?: boolean` (default `true`) — edit mode passes `false`
  when the issue has no block, so an issue without a pipeline starts with
  nothing selected (and therefore no spurious dirty state) instead of
  defaulting to `basic`.
- `footer?: ReactNode` — rendered at the bottom of the expanded section; the
  container uses it for the "Update Issue" button so the button sits inside
  the Pipeline card.

Create mode passes none of these and behaves exactly as today.

### 3. Render slot opens in edit mode (`KanbanIssuePanel.tsx`)

Change `{isCreateMode && renderPipeline && renderPipeline()}` to
`{renderPipeline && renderPipeline()}` and update the slot comment: the
container decides per-mode content (and can return `null`). Placement is
unchanged (after the description editor); in edit mode this puts the Pipeline
card between the description and the SpecKit/Relationships sections.

### 4. Container wires edit mode (`KanbanIssuePanelContainer.tsx`)

- `renderPipeline` now branches on mode:
  - **create**: exactly today's `<PipelineSection …/>` (keyed on composer +
    reset counter).
  - **edit**: `<PipelineSection key={'edit:' + issueId} initialBlock={extractPipelineBlock(selectedIssue.description)} seedDefaultPipeline={false} footer={<UpdateIssueButton/>} onChange={setEditPipelineSelection}/>`.
    Keyed on the issue id so switching issues reseeds.
- Edit-mode pipeline selection lives in `useState<PipelineSelection | null>`
  (create mode keeps its existing ref).
- **Dirty check**: `editPipelineSelection.block` (trimmed) differs from
  `extractPipelineBlock(latest description)` (trimmed). The "Update Issue"
  button renders enabled only when dirty (disabled otherwise), so idle issues
  show an inert button rather than a phantom pending change.
- **Apply (`handleUpdateIssuePipeline`)**:
  `updateIssue(issueId, { description: appendPipelineToDescription(latestDescriptionRef.current, block) || null })`,
  plus `dispatchFormState({type:'setEditDescription', …})` and
  `latestDescriptionRef.current = next` so the on-screen editor reflects the
  new description immediately. Deselecting everything yields an empty block,
  which `appendPipelineToDescription` treats as "strip the block" —
  removing a pipeline is therefore also supported.
- Uses `latestDescriptionRef` (not `selectedIssue.description`) as the base so
  in-flight debounced prose edits aren't reverted.

### 5. i18n

New keys under `taskPipeline` in all 7
`packages/web-core/src/i18n/locales/*/common.json`:

- `updateIssue` = "Update Issue" — the apply button label.
- `editModeDescription` — edit-mode helper copy explaining stages are stored
  in the issue description and applied on Update Issue.

## Edge cases

| Case | Behavior |
|---|---|
| Issue has no pipeline block | Section starts empty (no `basic` default), button disabled until something is selected |
| Block contains manual/custom lines | Preserved verbatim through recompose (existing `extractManualLines` path) |
| Block references stages/pipelines that no longer exist in the TOML files | Unmatched heading names dropped from selection; unmatched numbered lines survive as manual lines (existing behavior of `extractManualLines`) |
| Concurrent prose edits in the description editor | Apply merges onto `latestDescriptionRef`, so prose typed while the pipeline card is dirty is kept |
| Deselect all stages/pipelines then Update | Block stripped from the description entirely |
| Switching issues with the panel open | `key` remount reseeds from the newly selected issue |
| Pipelines list still loading | Section renders `null` (existing `pipelines.length === 0` guard); no seeding happens until loaded |

## Testing

- `taskPipeline.test.ts`: unit tests for `parsePipelineSelection`
  (round-trip with `composePipelineBlock`, bare heading, unknown names,
  manual lines ignored, shared stages deduped).
- `pnpm run check` and `pnpm run lint` must pass; run existing Vitest suite.

## Files touched

- `packages/web-core/src/shared/lib/pipeline/taskPipeline.ts` (+ tests)
- `packages/web-core/src/pages/kanban/PipelineSection.tsx`
- `packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`
- `packages/ui/src/components/KanbanIssuePanel.tsx`
- `packages/web-core/src/i18n/locales/*/common.json` (7 locales)
