import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CheckCircleIcon,
  CircleNotchIcon,
  PlusIcon,
  TrashIcon,
  WarningCircleIcon,
  ArrowClockwiseIcon,
  XIcon,
} from '@phosphor-icons/react';
import { Button } from '@vibe/ui/components/Button';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import type { PipelineFileStatus, PipelineParseError } from 'shared/types';
import {
  useDeletePipelineMutation,
  usePipelineRaw,
  usePipelineStatuses,
  useResetDefaultPipelinesMutation,
  useResetPipelineMutation,
  useValidatePipelineMutation,
  useWritePipelineRawMutation,
} from '@/shared/hooks/usePipelines';
import {
  createPipelineStarterToml,
  formatPipelineErrorLocation,
  isBundledPipelineId,
  isValidPipelineId,
  selectPipelineAfterRefresh,
  validationTupleMatches,
  type PipelineValidationTuple,
} from '@/shared/lib/pipeline/pipelineSettings';
import { cn } from '@/shared/lib/utils';
import { SettingsCard, SettingsInput } from './SettingsComponents';
import { useSettingsDirty } from './SettingsDirtyContext';
import { useSettingsMachineClient } from './SettingsHostContext';

type DraftKind = 'existing' | 'new';
type ValidationStatus = 'idle' | 'pending' | 'valid' | 'invalid';

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function scopeSignature(scopeKey: readonly string[]): string {
  return scopeKey.join('\u0000');
}

function parseErrorMessage(error: PipelineParseError): string {
  const location = formatPipelineErrorLocation(error);
  return location ? `${error.message} (${location})` : error.message;
}

function PipelineStatusRow({
  status,
  selected,
  onSelect,
}: {
  status: PipelineFileStatus;
  selected: boolean;
  onSelect: () => void;
}) {
  const { t } = useTranslation('settings');
  const location = formatPipelineErrorLocation(status.error);

  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        'w-full min-w-0 px-3 py-2 text-left transition-colors',
        'border-b border-border/60 last:border-b-0',
        selected ? 'bg-brand/10' : 'hover:bg-secondary/70'
      )}
    >
      <div className="flex min-w-0 items-start gap-2">
        {status.valid ? (
          <CheckCircleIcon
            className="mt-0.5 size-icon-sm shrink-0 text-success"
            weight="fill"
          />
        ) : (
          <WarningCircleIcon
            className="mt-0.5 size-icon-sm shrink-0 text-error"
            weight="fill"
          />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <code className="truncate font-mono text-sm text-high">
              {status.id}
            </code>
            <span
              className={cn(
                'rounded-sm px-1.5 py-0.5 text-xs',
                status.valid
                  ? 'bg-success/10 text-success'
                  : 'bg-error/10 text-error'
              )}
            >
              {status.valid
                ? t('settings.pipelines.status.valid')
                : t('settings.pipelines.status.invalid')}
            </span>
          </div>
          <div className="mt-1 truncate text-xs text-low">{status.name}</div>
          {status.valid ? (
            <div className="mt-1 text-xs text-low">
              {status.stage_count == null
                ? t('settings.pipelines.status.stageCountUnknown')
                : t('settings.pipelines.status.stageCount', {
                    count: status.stage_count,
                  })}
            </div>
          ) : status.error ? (
            <div className="mt-1 text-xs text-error">
              {status.error.message}
              {location ? (
                <span className="text-low">
                  {' '}
                  {t('settings.pipelines.status.location', {
                    location,
                  })}
                </span>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>
    </button>
  );
}

export function PipelinesSettingsSection() {
  const { t } = useTranslation('settings');
  const machineClient = useSettingsMachineClient();
  const scopeKey =
    machineClient?.queryScopeKey ?? (['machine', 'unselected'] as const);
  const scope = scopeSignature(scopeKey);
  const { setDirty: setContextDirty } = useSettingsDirty();
  const statusesQuery = usePipelineStatuses(machineClient);
  const statuses = statusesQuery.data ?? [];

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draftKind, setDraftKind] = useState<DraftKind | null>(null);
  const [draftContent, setDraftContent] = useState('');
  const [lastPersistedContent, setLastPersistedContent] = useState('');
  const [newPipelineId, setNewPipelineId] = useState('');
  const [addError, setAddError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const [validationStatus, setValidationStatus] =
    useState<ValidationStatus>('idle');
  const [validationError, setValidationError] =
    useState<PipelineParseError | null>(null);
  const [validationTuple, setValidationTuple] =
    useState<PipelineValidationTuple | null>(null);
  const latestTupleRef = useRef<PipelineValidationTuple | null>(null);

  const effectiveId = selectedId;
  const isDirty = draftKind !== null && draftContent !== lastPersistedContent;
  const selectedStatus = statuses.find((status) => status.id === selectedId);
  const rawQuery = usePipelineRaw(
    machineClient,
    draftKind === 'existing' ? selectedId : null
  );
  const validateMutation = useValidatePipelineMutation(machineClient);
  const writeMutation = useWritePipelineRawMutation(machineClient);
  const deleteMutation = useDeletePipelineMutation(machineClient);
  const resetMutation = useResetPipelineMutation(machineClient);
  const resetAllMutation = useResetDefaultPipelinesMutation(machineClient);
  const mutationPending =
    writeMutation.isPending ||
    deleteMutation.isPending ||
    resetMutation.isPending ||
    resetAllMutation.isPending;

  const resetValidation = useCallback(() => {
    setValidationStatus('idle');
    setValidationError(null);
    setValidationTuple(null);
    latestTupleRef.current = null;
  }, []);

  const seedEmpty = useCallback(() => {
    setSelectedId(null);
    setDraftKind(null);
    setDraftContent('');
    setLastPersistedContent('');
    setMutationError(null);
    setSaveSuccess(false);
    resetValidation();
  }, [resetValidation]);

  const confirmDiscard = useCallback(
    async (message?: string) => {
      if (!isDirty) {
        return true;
      }

      const result = await ConfirmDialog.show({
        title: t('settings.pipelines.confirm.discardTitle'),
        message: message ?? t('settings.pipelines.confirm.discardMessage'),
        confirmText: t('settings.pipelines.confirm.discardConfirm'),
        cancelText: t('settings.pipelines.confirm.cancel'),
        variant: 'destructive',
      });
      return result === 'confirmed';
    },
    [isDirty, t]
  );

  useEffect(() => {
    setContextDirty('pipelines', isDirty);
    return () => setContextDirty('pipelines', false);
  }, [isDirty, setContextDirty]);

  useEffect(() => {
    seedEmpty();
  }, [scope, seedEmpty]);

  useEffect(() => {
    if (!statusesQuery.isSuccess) {
      return;
    }

    setSelectedId((current) => {
      if (draftKind === 'new') {
        return current;
      }
      const next = selectPipelineAfterRefresh(statuses, current);
      if (!next) {
        setDraftKind(null);
      } else if (next !== current) {
        setDraftKind('existing');
      }
      return next;
    });
  }, [draftKind, statuses, statusesQuery.isSuccess]);

  useEffect(() => {
    if (selectedId && draftKind === null) {
      setDraftKind('existing');
    }
  }, [draftKind, selectedId]);

  useEffect(() => {
    if (
      draftKind !== 'existing' ||
      !selectedId ||
      rawQuery.isLoading ||
      rawQuery.isError ||
      rawQuery.data == null
    ) {
      return;
    }

    setDraftContent(rawQuery.data);
    setLastPersistedContent(rawQuery.data);
    setMutationError(null);
    setSaveSuccess(false);
    resetValidation();
  }, [
    draftKind,
    rawQuery.data,
    rawQuery.isError,
    rawQuery.isLoading,
    resetValidation,
    selectedId,
    scope,
  ]);

  const currentTuple = useCallback(
    (id: string, content: string): PipelineValidationTuple => ({
      scopeKey,
      id,
      content,
    }),
    [scopeKey]
  );

  const runValidation = useCallback(
    async (id: string, content: string) => {
      if (!machineClient) {
        return false;
      }

      const tuple = currentTuple(id, content);
      latestTupleRef.current = tuple;
      setValidationTuple(tuple);
      setValidationStatus('pending');
      setValidationError(null);

      try {
        const result = await validateMutation.mutateAsync({ id, content });
        if (!validationTupleMatches(latestTupleRef.current, tuple)) {
          return false;
        }
        setValidationStatus(result.valid ? 'valid' : 'invalid');
        setValidationError(result.error);
        return result.valid;
      } catch (err) {
        if (validationTupleMatches(latestTupleRef.current, tuple)) {
          setValidationStatus('invalid');
          setValidationError({
            message: errorText(err),
            line: null,
            column: null,
          });
        }
        return false;
      }
    },
    [currentTuple, machineClient, validateMutation]
  );

  useEffect(() => {
    if (!effectiveId || !machineClient || !isDirty) {
      resetValidation();
      return;
    }

    const tuple = currentTuple(effectiveId, draftContent);
    latestTupleRef.current = tuple;
    setValidationTuple(tuple);
    setValidationStatus('pending');
    setValidationError(null);

    const timeout = window.setTimeout(() => {
      void runValidation(effectiveId, draftContent);
    }, 400);

    return () => window.clearTimeout(timeout);
  }, [
    currentTuple,
    draftContent,
    effectiveId,
    isDirty,
    machineClient,
    resetValidation,
    runValidation,
  ]);

  const latestTupleIsValid = useMemo(() => {
    if (!effectiveId || validationStatus !== 'valid') {
      return false;
    }
    return validationTupleMatches(
      validationTuple,
      currentTuple(effectiveId, draftContent)
    );
  }, [
    currentTuple,
    draftContent,
    effectiveId,
    validationStatus,
    validationTuple,
  ]);

  const saveDisabled =
    !machineClient ||
    !effectiveId ||
    !isDirty ||
    mutationPending ||
    validationStatus === 'pending' ||
    !latestTupleIsValid;

  const handleSelect = async (id: string) => {
    if (id === selectedId) {
      return;
    }
    const ok = await confirmDiscard(
      t('settings.pipelines.confirm.switchFileMessage')
    );
    if (!ok) {
      return;
    }
    setSelectedId(id);
    setDraftKind('existing');
    setMutationError(null);
    setSaveSuccess(false);
    resetValidation();
  };

  const handleStartAdd = async () => {
    setAddError(null);
    const id = newPipelineId.trim();
    if (!isValidPipelineId(id)) {
      setAddError(t('settings.pipelines.add.invalidId'));
      return;
    }
    if (statuses.some((status) => status.id === id)) {
      setAddError(t('settings.pipelines.add.conflict'));
      return;
    }
    const ok = await confirmDiscard(t('settings.pipelines.confirm.addMessage'));
    if (!ok) {
      return;
    }
    const content = createPipelineStarterToml(id);
    setSelectedId(id);
    setDraftKind('new');
    setDraftContent(content);
    setLastPersistedContent('');
    setMutationError(null);
    setSaveSuccess(false);
    resetValidation();
  };

  const handleCloseNewDraft = async () => {
    if (draftKind !== 'new') {
      return;
    }
    const ok = await confirmDiscard(
      t('settings.pipelines.confirm.closeNewMessage')
    );
    if (!ok) {
      return;
    }
    const next = selectPipelineAfterRefresh(statuses, null);
    if (next) {
      setSelectedId(next);
      setDraftKind('existing');
    } else {
      seedEmpty();
    }
  };

  const handleDiscard = async () => {
    if (draftKind === 'new') {
      await handleCloseNewDraft();
      return;
    }
    if (rawQuery.data != null) {
      setDraftContent(rawQuery.data);
      setLastPersistedContent(rawQuery.data);
    }
    setMutationError(null);
    setSaveSuccess(false);
    resetValidation();
  };

  const handleSave = async () => {
    if (!machineClient || !effectiveId || !isDirty || mutationPending) {
      return;
    }

    setMutationError(null);
    setSaveSuccess(false);
    const valid = validationTupleMatches(
      validationTuple,
      currentTuple(effectiveId, draftContent)
    )
      ? validationStatus === 'valid'
      : await runValidation(effectiveId, draftContent);
    if (!valid) {
      return;
    }

    try {
      await writeMutation.mutateAsync({
        id: effectiveId,
        body: { content: draftContent },
      });
      setDraftKind('existing');
      setSelectedId(effectiveId);
      setLastPersistedContent(draftContent);
      setNewPipelineId('');
      setSaveSuccess(true);
      resetValidation();
      await statusesQuery.refetch();
    } catch (err) {
      setMutationError(errorText(err));
    }
  };

  const handleDelete = async () => {
    if (!machineClient || !selectedId || draftKind !== 'existing') {
      return;
    }
    const isFinal = statuses.length <= 1;
    const ok = await confirmDiscard(
      isDirty ? t('settings.pipelines.confirm.deleteDirtyMessage') : undefined
    );
    if (!ok) {
      return;
    }
    const result = await ConfirmDialog.show({
      title: t('settings.pipelines.confirm.deleteTitle', { id: selectedId }),
      message: isFinal
        ? t('settings.pipelines.confirm.deleteFinalMessage')
        : t('settings.pipelines.confirm.deleteMessage'),
      confirmText: t('settings.pipelines.actions.delete'),
      cancelText: t('settings.pipelines.confirm.cancel'),
      variant: 'destructive',
    });
    if (result !== 'confirmed') {
      return;
    }

    try {
      setMutationError(null);
      await deleteMutation.mutateAsync(selectedId);
      const remaining = statuses.filter((status) => status.id !== selectedId);
      const next = selectPipelineAfterRefresh(remaining, null);
      if (next) {
        setSelectedId(next);
        setDraftKind('existing');
      } else {
        seedEmpty();
      }
      await statusesQuery.refetch();
    } catch (err) {
      setMutationError(errorText(err));
    }
  };

  const handleResetOne = async () => {
    if (!machineClient || !selectedId || !isBundledPipelineId(selectedId)) {
      return;
    }
    const result = await ConfirmDialog.show({
      title: t('settings.pipelines.confirm.resetOneTitle', { id: selectedId }),
      message: isDirty
        ? t('settings.pipelines.confirm.resetOneDirtyMessage')
        : t('settings.pipelines.confirm.resetOneMessage'),
      confirmText: t('settings.pipelines.actions.resetOne'),
      cancelText: t('settings.pipelines.confirm.cancel'),
      variant: 'destructive',
    });
    if (result !== 'confirmed') {
      return;
    }

    try {
      setMutationError(null);
      await resetMutation.mutateAsync(selectedId);
      setDraftKind('existing');
      await statusesQuery.refetch();
      await rawQuery.refetch();
    } catch (err) {
      setMutationError(errorText(err));
    }
  };

  const handleResetAll = async () => {
    if (!machineClient) {
      return;
    }
    const overwritesOpen =
      selectedId != null &&
      (isBundledPipelineId(selectedId) || draftKind === 'new');
    const result = await ConfirmDialog.show({
      title: t('settings.pipelines.confirm.resetAllTitle'),
      message:
        isDirty && overwritesOpen
          ? t('settings.pipelines.confirm.resetAllDirtyMessage')
          : t('settings.pipelines.confirm.resetAllMessage'),
      confirmText: t('settings.pipelines.actions.resetAll'),
      cancelText: t('settings.pipelines.confirm.cancel'),
      variant: 'destructive',
    });
    if (result !== 'confirmed') {
      return;
    }

    try {
      setMutationError(null);
      await resetAllMutation.mutateAsync();
      setDraftKind('existing');
      await statusesQuery.refetch();
      await rawQuery.refetch();
    } catch (err) {
      setMutationError(errorText(err));
    }
  };

  const validationMessage = useMemo(() => {
    if (!isDirty) {
      return null;
    }
    if (validationStatus === 'pending') {
      return t('settings.pipelines.validation.pending');
    }
    if (validationStatus === 'valid') {
      return t('settings.pipelines.validation.valid');
    }
    if (validationStatus === 'invalid') {
      return validationError
        ? parseErrorMessage(validationError)
        : t('settings.pipelines.validation.invalid');
    }
    return null;
  }, [isDirty, t, validationError, validationStatus]);

  return (
    <SettingsCard
      title={t('settings.pipelines.title')}
      description={t('settings.pipelines.description')}
      headerAction={
        <Button
          type="button"
          size="sm"
          variant="outline"
          onClick={handleResetAll}
          disabled={!machineClient || mutationPending}
        >
          <ArrowClockwiseIcon className="mr-1 size-icon-xs" weight="bold" />
          {t('settings.pipelines.actions.resetAll')}
        </Button>
      }
    >
      {statusesQuery.isLoading && (
        <div className="flex items-center gap-2 text-sm text-low">
          <CircleNotchIcon className="size-icon-sm animate-spin" />
          {t('settings.pipelines.loading')}
        </div>
      )}

      {statusesQuery.isError && (
        <div className="rounded-sm border border-error/50 bg-error/10 p-3 text-sm text-error">
          {errorText(statusesQuery.error)}
        </div>
      )}

      {mutationError && (
        <div className="rounded-sm border border-error/50 bg-error/10 p-3 text-sm text-error">
          {mutationError}
        </div>
      )}

      {saveSuccess && (
        <div className="rounded-sm border border-success/50 bg-success/10 p-3 text-sm text-success">
          {t('settings.pipelines.save.success')}
        </div>
      )}

      <div className="grid min-h-[420px] min-w-0 grid-cols-1 overflow-hidden rounded-sm border border-border lg:grid-cols-[minmax(220px,280px)_1fr]">
        <div className="min-h-0 border-b border-border bg-panel lg:border-b-0 lg:border-r">
          <div className="border-b border-border bg-secondary/50 p-3">
            <div className="text-sm font-medium text-high">
              {t('settings.pipelines.list.title')}
            </div>
            <p className="mt-1 text-xs text-low">
              {t('settings.pipelines.list.description')}
            </p>
          </div>
          <div className="max-h-72 overflow-y-auto lg:max-h-[520px]">
            {!statusesQuery.isLoading &&
              !statusesQuery.isError &&
              statuses.length === 0 && (
                <div className="p-3 text-sm text-low">
                  {t('settings.pipelines.empty')}
                </div>
              )}
            {statuses.map((status) => (
              <PipelineStatusRow
                key={status.id}
                status={status}
                selected={status.id === selectedId}
                onSelect={() => void handleSelect(status.id)}
              />
            ))}
          </div>
          <div className="space-y-2 border-t border-border p-3">
            <div className="flex gap-2">
              <SettingsInput
                value={newPipelineId}
                onChange={(value) => {
                  setNewPipelineId(value);
                  setAddError(null);
                }}
                placeholder={t('settings.pipelines.add.placeholder')}
                disabled={mutationPending}
                error={Boolean(addError)}
              />
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => void handleStartAdd()}
                disabled={mutationPending}
                aria-label={t('settings.pipelines.actions.add')}
                title={t('settings.pipelines.actions.add')}
              >
                <PlusIcon className="size-icon-xs" weight="bold" />
              </Button>
            </div>
            {addError ? (
              <p className="text-xs text-error">{addError}</p>
            ) : (
              <p className="text-xs text-low">
                {t('settings.pipelines.add.helper')}
              </p>
            )}
          </div>
        </div>

        <div className="flex min-h-0 min-w-0 flex-col bg-panel">
          <div className="flex min-w-0 flex-col gap-3 border-b border-border bg-secondary/50 p-3 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0">
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                <span className="text-sm font-medium text-high">
                  {effectiveId ?? t('settings.pipelines.editor.noSelection')}
                </span>
                {draftKind === 'new' && (
                  <span className="rounded-sm bg-brand/10 px-1.5 py-0.5 text-xs text-brand">
                    {t('settings.pipelines.editor.newDraft')}
                  </span>
                )}
              </div>
              {selectedStatus?.error && (
                <p className="mt-1 text-xs text-error">
                  {parseErrorMessage(selectedStatus.error)}
                </p>
              )}
            </div>
            <div className="flex shrink-0 flex-wrap gap-2">
              {draftKind === 'new' && (
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  onClick={() => void handleCloseNewDraft()}
                  disabled={mutationPending}
                >
                  <XIcon className="mr-1 size-icon-xs" weight="bold" />
                  {t('settings.pipelines.actions.closeDraft')}
                </Button>
              )}
              {selectedId &&
                draftKind === 'existing' &&
                isBundledPipelineId(selectedId) && (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() => void handleResetOne()}
                    disabled={mutationPending}
                  >
                    <ArrowClockwiseIcon
                      className="mr-1 size-icon-xs"
                      weight="bold"
                    />
                    {t('settings.pipelines.actions.resetOne')}
                  </Button>
                )}
              {draftKind === 'existing' && (
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  onClick={() => void handleDelete()}
                  disabled={!selectedId || mutationPending}
                >
                  <TrashIcon className="mr-1 size-icon-xs" weight="bold" />
                  {t('settings.pipelines.actions.delete')}
                </Button>
              )}
            </div>
          </div>

          <div className="flex min-h-0 flex-1 flex-col gap-3 p-3">
            {rawQuery.isLoading && draftKind === 'existing' && (
              <div className="flex items-center gap-2 text-sm text-low">
                <CircleNotchIcon className="size-icon-sm animate-spin" />
                {t('settings.pipelines.editor.loadingRaw')}
              </div>
            )}

            {rawQuery.isError && draftKind === 'existing' && (
              <div className="rounded-sm border border-error/50 bg-error/10 p-3 text-sm text-error">
                {errorText(rawQuery.error)}
              </div>
            )}

            {draftKind === null && !statusesQuery.isLoading ? (
              <div className="rounded-sm border border-border bg-secondary/50 p-4 text-sm text-low">
                {t('settings.pipelines.editor.empty')}
              </div>
            ) : (
              <textarea
                value={draftContent}
                onChange={(event) => {
                  setDraftContent(event.target.value);
                  setMutationError(null);
                  setSaveSuccess(false);
                }}
                spellCheck={false}
                disabled={mutationPending || rawQuery.isLoading}
                className={cn(
                  'min-h-[300px] flex-1 resize-y rounded-sm border border-border bg-secondary px-3 py-2',
                  'font-mono text-sm leading-5 text-high placeholder:text-low',
                  'focus:outline-none focus:ring-1 focus:ring-brand',
                  'lg:min-h-0',
                  (mutationPending || rawQuery.isLoading) &&
                    'cursor-not-allowed opacity-60'
                )}
                placeholder={t('settings.pipelines.editor.placeholder')}
              />
            )}

            {validationMessage && (
              <div
                className={cn(
                  'rounded-sm border p-2 text-sm',
                  validationStatus === 'valid'
                    ? 'border-success/50 bg-success/10 text-success'
                    : validationStatus === 'pending'
                      ? 'border-border bg-secondary text-low'
                      : 'border-error/50 bg-error/10 text-error'
                )}
              >
                {validationStatus === 'pending' && (
                  <CircleNotchIcon className="mr-1 inline size-icon-xs animate-spin" />
                )}
                {validationMessage}
              </div>
            )}
          </div>

          <div className="flex flex-col gap-2 border-t border-border p-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="text-xs text-low">
              {isDirty
                ? t('settings.pipelines.editor.dirty')
                : t('settings.pipelines.editor.clean')}
            </div>
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                size="sm"
                variant="ghost"
                onClick={() => void handleDiscard()}
                disabled={!isDirty || mutationPending}
              >
                {t('settings.pipelines.actions.discard')}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => void handleSave()}
                disabled={saveDisabled}
              >
                {writeMutation.isPending && (
                  <CircleNotchIcon className="mr-1 size-icon-xs animate-spin" />
                )}
                {t('settings.pipelines.actions.save')}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </SettingsCard>
  );
}
