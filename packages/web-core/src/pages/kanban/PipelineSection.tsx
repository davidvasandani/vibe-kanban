import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { CaretDownIcon, CaretRightIcon } from '@phosphor-icons/react';
import type { Pipeline } from 'shared/types';
import { usePipelines } from '@/shared/hooks/usePipelines';
import {
  canonicalStageOrder,
  composePipelineBlock,
  extractManualLines,
  orderedEnabledStages,
  parsePipelineSelection,
} from '@/shared/lib/pipeline/taskPipeline';

export interface PipelineSelection {
  /** Selected pipeline ids (additive; empty when nothing is chosen). */
  pipelineIds: string[];
  /** Ticked stage ids, in canonical merge order. */
  enabledIds: string[];
  /** The operator's manual/extra text, extracted from the composed block. */
  customText: string;
  /** The composed `## Pipeline` markdown block (empty when nothing selected). */
  block: string;
}

/** The default-enabled stage ids across the given pipelines, deduped. */
function defaultEnabledUnion(pipelines: readonly Pipeline[]): Set<string> {
  return new Set(
    pipelines.flatMap((p) =>
      p.stages.filter((s) => s.default_enabled).map((s) => s.id)
    )
  );
}

interface PipelineSectionProps {
  /** Disabled while the task is being submitted. */
  disabled?: boolean;
  /** Emits the current selection whenever it changes. */
  onChange: (selection: PipelineSelection) => void;
  /**
   * Seed the selection from an existing `## Pipeline` block (edit mode):
   * the block's pipelines/stages are pre-selected and its text (incl.
   * manual lines) becomes the initial composed block. Seeding happens once,
   * after the pipelines list loads — remount (via `key`) to reseed.
   */
  initialBlock?: string;
  /**
   * Whether to default the picker to the `basic` pipeline when there is no
   * `initialBlock` (create-mode behavior). Edit mode passes `false` so an
   * issue without a pipeline starts with nothing selected.
   */
  seedDefaultPipeline?: boolean;
  /** Overrides the create-mode helper copy under the section title. */
  helperText?: string;
  /** Rendered at the bottom of the expanded card (e.g. an apply button). */
  footer?: ReactNode;
}

/**
 * Per-task "Pipeline" control for the task-create and issue-edit flows.
 * Fetches the file-based pipelines, lets the operator additively pick one or
 * more and tick which of their (deduped, canonically-ordered) stages apply,
 * and edit the composed prompt block. Recompose is non-destructive: any
 * manual lines the operator typed into the block survive further
 * tick/selection changes. Emits the result so the container can append it to
 * the task description (create) or apply it via Update Issue (edit, seeded
 * from the issue's existing block via `initialBlock`).
 */
export function PipelineSection({
  disabled,
  onChange,
  initialBlock,
  seedDefaultPipeline = true,
  helperText,
  footer,
}: PipelineSectionProps) {
  const { t } = useTranslation('common');

  const { data: pipelines = [] } = usePipelines();
  const [expanded, setExpanded] = useState(true);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [enabledIds, setEnabledIds] = useState<Set<string>>(() => new Set());
  // The composed block, incl. delimiters. Regenerated (non-destructively)
  // whenever the selection/ticks change.
  const [text, setText] = useState('');

  // Seed the selection once the pipelines list loads, exactly once: from
  // `initialBlock` when given (edit mode), else the `basic`/first default
  // (create mode), else nothing. Ticks are set here and on pipeline toggle
  // (`togglePipeline`), never by a selection-watching effect — an effect
  // can't tell a user toggle from this seeding and would clobber the parsed
  // ticks with the `default_enabled` union.
  const appliedDefaultRef = useRef(false);
  useEffect(() => {
    if (appliedDefaultRef.current || pipelines.length === 0) return;
    appliedDefaultRef.current = true;
    if (initialBlock) {
      const parsed = parsePipelineSelection(initialBlock, pipelines);
      setSelectedIds(parsed.pipelineIds);
      setEnabledIds(new Set(parsed.enabledIds));
      // The block itself is the initial text, so its manual lines survive
      // the non-destructive recompose.
      setText(initialBlock);
      return;
    }
    if (!seedDefaultPipeline) return;
    const def = pipelines.find((p) => p.id === 'basic') ?? pipelines[0] ?? null;
    setSelectedIds(def ? [def.id] : []);
    setEnabledIds(defaultEnabledUnion(def ? [def] : []));
  }, [pipelines, initialBlock, seedDefaultPipeline]);

  const selectedPipelines = useMemo(
    () =>
      selectedIds
        .map((id) => pipelines.find((p) => p.id === id))
        .filter((p): p is Pipeline => p != null),
    [pipelines, selectedIds]
  );

  const orderedSteps = useMemo(
    () => canonicalStageOrder(selectedPipelines),
    [selectedPipelines]
  );

  // Fragments of ALL available pipelines' stages (not just selected ones),
  // so a generated stage line is recognised and dropped when its stage or
  // whole pipeline is deselected, instead of being stranded as "manual".
  const allFragments = useMemo(
    () =>
      new Set(pipelines.flatMap((p) => p.stages.map((s) => s.prompt_fragment))),
    [pipelines]
  );

  // Non-destructive recompose: read the previous text via the functional
  // updater (no `text` dep, so this can't loop) and preserve any manual
  // lines already present in it.
  useEffect(() => {
    setText((prev) =>
      composePipelineBlock(selectedPipelines, enabledIds, '', null, {
        previousBlock: prev,
        knownStageFragments: allFragments,
      })
    );
  }, [selectedPipelines, enabledIds, allFragments]);

  // Notify the parent of the effective selection whenever it settles.
  const emittedRef = useRef<string | null>(null);
  useEffect(() => {
    const block = text.trim();
    const signature = `${[...selectedIds].sort().join(',')}|${[...enabledIds].sort().join(',')}|${block}`;
    if (emittedRef.current === signature) return;
    emittedRef.current = signature;
    const customText = extractManualLines(block, allFragments).join('\n');
    onChange({
      pipelineIds: selectedIds,
      enabledIds: orderedEnabledStages(selectedPipelines, enabledIds).map(
        (s) => s.id
      ),
      customText,
      block,
    });
  }, [
    selectedIds,
    selectedPipelines,
    enabledIds,
    text,
    allFragments,
    onChange,
  ]);

  const togglePipeline = useCallback(
    (id: string) => {
      const next = selectedIds.includes(id)
        ? selectedIds.filter((p) => p !== id)
        : [...selectedIds, id];
      setSelectedIds(next);
      // Reseed the ticks to the default-enabled union of the new selection.
      setEnabledIds(
        defaultEnabledUnion(pipelines.filter((p) => next.includes(p.id)))
      );
    },
    [selectedIds, pipelines]
  );

  const toggleStep = useCallback((id: string) => {
    setEnabledIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const resetToGenerated = useCallback(() => {
    setText(composePipelineBlock(selectedPipelines, enabledIds, '', null));
  }, [selectedPipelines, enabledIds]);

  if (pipelines.length === 0) return null;

  return (
    <div className="p-base border-t space-y-base">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex items-center gap-half text-sm font-medium text-high"
      >
        {expanded ? (
          <CaretDownIcon className="size-icon-sm" weight="bold" />
        ) : (
          <CaretRightIcon className="size-icon-sm" weight="bold" />
        )}
        {t('taskPipeline.title')}
      </button>

      {expanded && (
        <>
          <p className="text-xs text-low">
            {helperText ?? t('taskPipeline.description')}
          </p>

          <div className="space-y-half">
            <label className="text-xs text-low block">
              {t('taskPipeline.pipelinesLabel')}
            </label>
            <div className="flex flex-col gap-half">
              {pipelines.map((p) => (
                <label
                  key={p.id}
                  className="flex items-start gap-half text-sm text-normal"
                >
                  <input
                    type="checkbox"
                    checked={selectedIds.includes(p.id)}
                    disabled={disabled}
                    onChange={() => togglePipeline(p.id)}
                    className="mt-0.5"
                  />
                  <span>
                    <span className="block">{p.name}</span>
                    {p.description && (
                      <span className="block text-xs text-low">
                        {p.description}
                      </span>
                    )}
                  </span>
                </label>
              ))}
            </div>
            <p className="text-xs text-low">
              {t('taskPipeline.pipelinesHelper')}
            </p>
          </div>

          {selectedPipelines.length > 0 && orderedSteps.length === 0 ? (
            <p className="text-xs text-low">{t('taskPipeline.noSteps')}</p>
          ) : orderedSteps.length > 0 ? (
            <div className="flex flex-col gap-half">
              {orderedSteps.map((step) => (
                <label
                  key={step.id}
                  className="flex items-center gap-half text-sm text-normal"
                >
                  <input
                    type="checkbox"
                    checked={enabledIds.has(step.id)}
                    disabled={disabled}
                    onChange={() => toggleStep(step.id)}
                  />
                  <span>
                    {step.label}
                    {step.heavy && (
                      <span className="ml-half rounded-sm border px-half text-xs text-low">
                        {t('taskPipeline.heavyBadge')}
                      </span>
                    )}
                  </span>
                </label>
              ))}
            </div>
          ) : null}

          <div className="space-y-half">
            <div className="flex items-center justify-between">
              <label className="text-xs text-low">
                {t('taskPipeline.addonLabel')}
              </label>
              <button
                type="button"
                onClick={resetToGenerated}
                disabled={disabled}
                className="text-xs text-brand hover:underline disabled:opacity-50"
              >
                {t('taskPipeline.resetToGenerated')}
              </button>
            </div>
            <textarea
              value={text}
              disabled={disabled}
              rows={6}
              onChange={(e) => setText(e.target.value)}
              placeholder={t('taskPipeline.addonPlaceholder')}
              className="w-full rounded-sm border bg-panel/40 px-half py-half text-sm text-high font-mono resize-y disabled:opacity-50"
            />
          </div>

          {footer}
        </>
      )}
    </div>
  );
}
