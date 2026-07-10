# Implementation Plan: Edit-mode Pipeline editing with "Update Issue" button (task vk/77eb-vk-pipeline)

Step-by-step build order. Rationale and edge-case matrix in `SPEC.md`;
prior-art recall in `PRIOR_KNOWLEDGE.md`. SpecKit artifacts (spec/plan/tasks)
live under `homelab/specs/vk/77eb-vk-pipeline/` once generated.

## Step 1 — `parsePipelineSelection` helper + unit tests

File: `packages/web-core/src/shared/lib/pipeline/taskPipeline.ts` (+
`taskPipeline.test.ts` alongside).

1. Export `parsePipelineSelection(block, pipelines) → { pipelineIds, enabledIds }`:
   - Slice the inner block text (reuse the `PIPELINE_START`/`PIPELINE_END` +
     heading fallback logic already in `extractPipelineBlockText`).
   - Heading `## Pipeline: A + B` → split remainder on `" + "`, match each
     name against `pipelines[].name` (first match), keep matches in heading
     order, dedupe. Bare `## Pipeline`/no heading → `[]`.
   - Numbered lines `N. <rest>`: build a `prompt_fragment → stage id` map
     across all `pipelines[].stages`; collect matching ids in order, dedupe.
     Non-matching numbered lines are ignored (they're manual text).
2. Tests: round-trip (`composePipelineBlock` → `parsePipelineSelection`
   returns the same selection), multi-pipeline heading, unknown pipeline
   name dropped, manual/unknown numbered lines ignored, stage shared by two
   pipelines returned once, empty/no-block input → empty selection.

No dependencies; pure function. Verify: `pnpm --filter web-core test` (or the
repo's vitest invocation for `taskPipeline.test.ts`).

## Step 2 — `PipelineSection` seeding + footer props

File: `packages/web-core/src/pages/kanban/PipelineSection.tsx`.

1. Add props `initialBlock?: string`, `seedDefaultPipeline?: boolean`
   (default `true`), `footer?: ReactNode`.
2. In the once-only seed effect (currently defaults to `basic`):
   - If `initialBlock` is non-empty: `parsePipelineSelection(initialBlock,
     pipelines)` → `setSelectedIds`, `setEnabledIds`, `setText(initialBlock)`;
     set a `seededFromBlockRef` so the "reseed ticks when selection changes"
     effect skips exactly one run (otherwise it clobbers parsed ticks with
     the `default_enabled` union).
   - Else if `seedDefaultPipeline`: today's behavior (`basic` or first).
   - Else: leave everything empty.
3. Render `{footer}` at the bottom of the expanded content.
4. The section is remount-keyed by callers, so no reseed-on-prop-change
   logic is needed.

Depends on Step 1. Verify: `pnpm run check` (types), create-mode behavior
unchanged by default props.

## Step 3 — Open the panel slot in edit mode

File: `packages/ui/src/components/KanbanIssuePanel.tsx`.

- `{isCreateMode && renderPipeline && renderPipeline()}` →
  `{renderPipeline && renderPipeline()}`; update the slot comment (container
  decides per-mode content; renders `null` when inapplicable). Same slot
  position ⇒ no border flip needed (card draws its own `border-t`).

Independent of Steps 1–2 (safe because the current container only returns
content in create mode until Step 4 lands, and this PR lands them together).

## Step 4 — Container: edit-mode wiring + Update Issue button

File: `packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx`.

1. Edit-mode selection state:
   `const [editPipelineSelection, setEditPipelineSelection] = useState<PipelineSelection | null>(null)`;
   handler `handleEditPipelineChange` stores every emission (including empty
   block, which means "cleared").
2. Seed block: `const issuePipelineBlock = extractPipelineBlock(selectedIssue?.description)`
   computed for edit mode.
3. Dirty check (memo): normalize both sides with `.trim()`;
   `dirty = editPipelineSelection != null && editPipelineSelection.block.trim() !== extractPipelineBlock(latest description).trim()`.
   Use the *live* description (local edit state / `displayData.description`)
   so a prose edit that deletes the block is compared against correctly.
4. Apply handler:
   ```ts
   const next = appendPipelineToDescription(
     latestDescriptionRef.current, editPipelineSelection.block) || null;
   updateIssue(selectedKanbanIssueId, { description: next });
   dispatchFormState({ type: 'setEditDescription', description: next });
   latestDescriptionRef.current = next;
   ```
   Cancel the pending debounced description save first
   (`cancelDebouncedDescription()`) so a stale debounce doesn't overwrite the
   applied description.
5. `renderPipeline` branches on mode; edit mode renders
   `<PipelineSection key={'edit:' + selectedKanbanIssueId} initialBlock={issuePipelineBlock} seedDefaultPipeline={false} disabled={false} footer={<update button/>} onChange={handleEditPipelineChange}/>`.
   The footer button: `PrimaryButton`-style, label
   `t('taskPipeline.updateIssue')`, `disabled={!dirty}`.
6. Reset `editPipelineSelection` to `null` when `selectedKanbanIssueId`
   changes (effect), matching the keyed remount.

Depends on Steps 1–3.

## Step 5 — i18n

Add to `taskPipeline` in all 7 locales
(`packages/web-core/src/i18n/locales/{en,es,fr,ja,ko,zh-Hans,zh-Hant}/common.json`):

- `updateIssue`: "Update Issue" (translated per locale).
- `editModeDescription`: helper copy for edit mode ("Stages are stored in
  the issue description; click Update Issue to apply changes.") — used in
  place of the create-mode `description` string when `initialBlock`/edit
  mode is active (pass a `description` override or a `mode` hint via the
  existing `description` copy; simplest: new optional `helperText` prop —
  decide at implementation, keep create-mode copy untouched).

## Step 6 — Verification

1. `pnpm run check` — web + Rust type checks.
2. `pnpm run lint`.
3. Vitest for `taskPipeline.test.ts`.
4. `pnpm run format` before finishing (repo rule).
5. Manual/E2E-ish sanity via `/verify`-style run if feasible: open an
   existing issue → Pipeline card shows current stages → tick/untick →
   Update Issue → description block updates; issue without pipeline starts
   empty; deselect-all strips the block.

## Risks / watchpoints

- The reseed-ticks effect in `PipelineSection` is keyed on `selectedIds`
  only (deliberate eslint-disable); the skip-once ref must be set *before*
  the seeding `setSelectedIds` call takes effect.
- `pipelines.length === 0` renders `null` — the seed effect must still be
  safe when pipelines load after mount (it already waits for
  `pipelines.length > 0`).
- Don't regress create mode: default prop values must reproduce today's
  behavior byte-for-byte (default `basic` selection, same emissions).
- The debounced-save interplay (step 4.4): always cancel before applying.
