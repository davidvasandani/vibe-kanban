import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CaretDownIcon, CaretRightIcon } from '@phosphor-icons/react';
import type { Pipeline } from 'shared/types';
import { usePipelines } from '@/shared/hooks/usePipelines';
import {
  canonicalStageOrder,
  composePipelineBlock,
  extractManualLines,
  orderedEnabledStages,
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

interface PipelineSectionProps {
  /** Disabled while the task is being submitted. */
  disabled?: boolean;
  /** Emits the current selection whenever it changes. */
  onChange: (selection: PipelineSelection) => void;
}

/**
 * Per-task "Pipeline" control for the task-create flow. Fetches the
 * file-based pipelines, lets the operator additively pick one or more and
 * tick which of their (deduped, canonically-ordered) stages apply, and edit
 * the composed prompt block. Recompose is non-destructive: any manual lines
 * the operator typed into the block survive further tick/selection changes.
 * Emits the result so the container can append it to the task description.
 */
export function PipelineSection({ disabled, onChange }: PipelineSectionProps) {
  const { t } = useTranslation('common');

  const { data: pipelines = [] } = usePipelines();
  const [expanded, setExpanded] = useState(true);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [enabledIds, setEnabledIds] = useState<Set<string>>(() => new Set());
  // The composed block, incl. delimiters. Regenerated (non-destructively)
  // whenever the selection/ticks change.
  const [text, setText] = useState('');

  // Default the picker to `basic` (else the first pipeline) once the list
  // loads, exactly once.
  const appliedDefaultRef = useRef(false);
  useEffect(() => {
    if (appliedDefaultRef.current || pipelines.length === 0) return;
    appliedDefaultRef.current = true;
    const def = pipelines.find((p) => p.id === 'basic') ?? pipelines[0] ?? null;
    setSelectedIds(def ? [def.id] : []);
  }, [pipelines]);

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

  // Reseed the ticks to the default-enabled union of the selected pipelines
  // whenever the pipeline *selection* changes (not whenever `pipelines`
  // refetches).
  useEffect(() => {
    setEnabledIds(
      new Set(
        selectedPipelines.flatMap((p) =>
          p.stages.filter((s) => s.default_enabled).map((s) => s.id)
        )
      )
    );
    // Deliberately keyed on `selectedIds` only: this reseeds ticks whenever
    // the pipeline *selection* changes, not whenever `pipelines` reloads.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedIds]);

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

  const togglePipeline = useCallback((id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((p) => p !== id) : [...prev, id]
    );
  }, []);

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
          <p className="text-xs text-low">{t('taskPipeline.description')}</p>

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
        </>
      )}
    </div>
  );
}
