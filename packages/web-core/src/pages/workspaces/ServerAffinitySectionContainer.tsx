import { useEffect, useMemo, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@vibe/ui/components/Select';
import {
  WorkspaceAffinityUpdateOutcome,
  WorkspacePlacementState,
} from 'shared/types';
import { ApiError, workerNodesApi, workspacesApi } from '@/shared/lib/api';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { isWorkerEligible } from '@/shared/lib/workerPlacement';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import {
  AUTOMATIC_PLACEMENT,
  COORDINATOR_PLACEMENT,
  serializeWorkspacePlacement,
} from '@/shared/lib/workspacePlacement';

export function ServerAffinitySectionContainer({
  workspaceId,
  isRunning,
}: {
  workspaceId: string;
  isRunning: boolean;
}) {
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const hostId = useHostId();
  const { executionProcessesAll } = useExecutionProcessesContext();
  const { data: placement } = useQuery({
    queryKey: ['workspacePlacement', hostId, workspaceId],
    queryFn: () => workspacesApi.getPlacement(workspaceId),
  });
  const { data: workers = [] } = useQuery({
    queryKey: ['workerNodes', hostId],
    queryFn: workerNodesApi.list,
    refetchInterval: 10_000,
  });
  const currentValue =
    placement?.placement_state === WorkspacePlacementState.local
      ? COORDINATOR_PLACEMENT
      : (placement?.requested_worker_node_id ?? AUTOMATIC_PLACEMENT);
  const [value, setValue] = useState(currentValue);
  const operationIds = useRef(new Map<string, string>());

  const requiredExecutorProfile = useMemo(() => {
    const process = executionProcessesAll
      .filter((candidate) => candidate.run_reason === 'codingagent')
      .sort((left, right) =>
        right.created_at.localeCompare(left.created_at)
      )[0];
    const action = process?.executor_action.typ;
    if (
      !action ||
      (action.type !== 'CodingAgentInitialRequest' &&
        action.type !== 'CodingAgentFollowUpRequest')
    ) {
      return undefined;
    }
    return action.executor_config.variant
      ? `${action.executor_config.executor}:${action.executor_config.variant}`
      : action.executor_config.executor;
  }, [executionProcessesAll]);

  useEffect(() => setValue(currentValue), [currentValue]);

  const currentWorker = workers.find(
    (worker) => worker.id === placement?.worker_node_id
  );
  const currentLabel =
    currentWorker?.hostname ??
    (placement?.placement_state === WorkspacePlacementState.local
      ? t('workspaces.serverAffinity.local')
      : t('workspaces.serverAffinity.unassigned'));
  const isLocal =
    !placement || placement.placement_state === WorkspacePlacementState.local;

  const mutation = useMutation({
    mutationFn: ({
      target,
      restart,
      operationId,
    }: {
      target: string;
      restart: boolean;
      operationId?: string;
    }) =>
      workspacesApi.updateAffinity(workspaceId, {
        ...serializeWorkspacePlacement(target),
        restart_running: restart,
        operation_id: restart ? (operationId ?? null) : null,
      }),
    onSuccess: async (result, variables) => {
      operationIds.current.delete(variables.target);
      queryClient.setQueryData(
        ['workspacePlacement', hostId, workspaceId],
        result.placement
      );
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ['workerNodes'] }),
        queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all }),
        queryClient.invalidateQueries({ queryKey: ['executionProcesses'] }),
      ]);
      if (result.outcome === WorkspaceAffinityUpdateOutcome.restart_failed) {
        toast.error(
          result.message ?? t('workspaces.serverAffinity.restartFailed')
        );
      } else {
        toast.success(t('workspaces.serverAffinity.updated'));
      }
    },
    onError: async (error, variables) => {
      // An HTTP response is conclusive and may represent a durably failed
      // operation. Transport failures keep the id for safe replay.
      if (error instanceof ApiError) {
        operationIds.current.delete(variables.target);
      }
      const message = error instanceof Error ? error.message : String(error);
      if (
        !variables.restart &&
        message.includes('confirm stop, migrate, and restart')
      ) {
        const confirmed = await ConfirmDialog.show({
          title: t('workspaces.serverAffinity.confirmTitle'),
          message: t('workspaces.serverAffinity.confirmMessage'),
          confirmText: t('workspaces.serverAffinity.confirmAction'),
          variant: 'destructive',
        });
        if (confirmed === 'confirmed') {
          const operationId =
            operationIds.current.get(variables.target) ?? crypto.randomUUID();
          operationIds.current.set(variables.target, operationId);
          mutation.mutate({
            target: variables.target,
            restart: true,
            operationId,
          });
          return;
        }
        setValue(currentValue);
        return;
      }
      setValue(currentValue);
      toast.error(
        error instanceof Error
          ? error.message
          : t('workspaces.serverAffinity.updateFailed')
      );
    },
  });

  const options = useMemo(
    () =>
      workers.map((worker) => ({
        worker,
        eligible: isWorkerEligible(worker, Date.now(), requiredExecutorProfile),
      })),
    [requiredExecutorProfile, workers]
  );

  const changeAffinity = async (target: string) => {
    setValue(target);
    if (target === currentValue) return;
    const requiresRestart = isRunning && target !== placement?.worker_node_id;
    if (requiresRestart) {
      const confirmed = await ConfirmDialog.show({
        title: t('workspaces.serverAffinity.confirmTitle'),
        message: t('workspaces.serverAffinity.confirmMessage'),
        confirmText: t('workspaces.serverAffinity.confirmAction'),
        variant: 'destructive',
      });
      if (confirmed !== 'confirmed') {
        setValue(currentValue);
        return;
      }
    }
    const operationId = requiresRestart
      ? (operationIds.current.get(target) ?? crypto.randomUUID())
      : undefined;
    if (operationId) operationIds.current.set(target, operationId);
    mutation.mutate({ target, restart: requiresRestart, operationId });
  };

  return (
    <div className="flex w-full flex-col gap-base p-base text-sm">
      <div className="flex items-center justify-between gap-base">
        <span className="text-low">
          {t('workspaces.serverAffinity.current')}
        </span>
        <span className="truncate text-normal" title={currentLabel}>
          {currentLabel}
        </span>
      </div>
      {isLocal ? (
        <p className="text-low">
          {t('workspaces.serverAffinity.localDescription')}
        </p>
      ) : (
        <div className="flex items-center justify-between gap-base">
          <span className="text-low">
            {t('workspaces.serverAffinity.runOn')}
          </span>
          <Select
            value={value}
            onValueChange={(target) => void changeAffinity(target)}
            disabled={mutation.isPending}
          >
            <SelectTrigger className="h-8 min-w-40 max-w-52">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={AUTOMATIC_PLACEMENT}>
                {t('workspaces.serverAffinity.automatic')}
              </SelectItem>
              <SelectItem value={COORDINATOR_PLACEMENT}>
                {t('workspaces.serverAffinity.coordinator')}
              </SelectItem>
              {options.map(({ worker, eligible }) => (
                <SelectItem
                  key={worker.id}
                  value={worker.id}
                  disabled={!eligible && worker.id !== placement.worker_node_id}
                >
                  {worker.hostname}
                  {!eligible
                    ? ` · ${t('workspaces.serverAffinity.unavailable')}`
                    : ''}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
    </div>
  );
}
