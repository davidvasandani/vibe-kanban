import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { WorkerMountStatus, WorkerNodeStatus } from 'shared/types';
import { workerNodesApi } from '@/shared/lib/api';
import { SettingsCard } from './SettingsComponents';

const WORKER_NODES_QUERY_KEY = ['worker-nodes'] as const;

function numberField(value: unknown, key: string): number | null {
  if (!value || typeof value !== 'object') return null;
  const field = (value as Record<string, unknown>)[key];
  return typeof field === 'number' ? field : null;
}

export function WorkersSettingsSection() {
  const { t } = useTranslation(['settings']);
  const queryClient = useQueryClient();
  const workers = useQuery({
    queryKey: WORKER_NODES_QUERY_KEY,
    queryFn: workerNodesApi.list,
    refetchInterval: 10_000,
  });
  const drain = useMutation({
    mutationFn: ({
      workerNodeId,
      draining,
    }: {
      workerNodeId: string;
      draining: boolean;
    }) => workerNodesApi.setDraining(workerNodeId, draining),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: WORKER_NODES_QUERY_KEY }),
  });

  return (
    <SettingsCard
      title={t('settings.workers.title', 'Cluster workers')}
      description={t(
        'settings.workers.description',
        'Worker health, shared storage, capacity, and scheduling state.'
      )}
    >
      {workers.isLoading && (
        <p className="text-sm text-low">
          {t('settings.workers.loading', 'Loading workers…')}
        </p>
      )}
      {workers.isError && (
        <p className="text-sm text-error">
          {t('settings.workers.loadError', 'Unable to load cluster workers.')}
        </p>
      )}
      {workers.data?.length === 0 && (
        <p className="text-sm text-low">
          {t('settings.workers.empty', 'No workers have registered yet.')}
        </p>
      )}
      <div className="space-y-3">
        {workers.data?.map((worker) => {
          const active =
            numberField(worker.resource_snapshot, 'active_execution_count') ??
            0;
          const load = numberField(worker.resource_snapshot, 'load_1m');
          const schedulable =
            worker.status === WorkerNodeStatus.online &&
            worker.mount_status === WorkerMountStatus.healthy;
          return (
            <div
              key={worker.id}
              className="rounded-sm border border-border bg-secondary/20 p-3"
            >
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="font-medium text-high">{worker.hostname}</div>
                  <div className="mt-1 text-xs text-low">
                    {schedulable
                      ? t('settings.workers.schedulable', 'Schedulable')
                      : t(
                          'settings.workers.unschedulable',
                          'Not schedulable'
                        )}{' '}
                    · {worker.status} · {worker.mount_status}
                  </div>
                  <div className="mt-1 text-xs text-low">
                    {t('settings.workers.active', 'Active executions')}:{' '}
                    {active}
                    {load === null
                      ? ''
                      : ` · ${t('settings.workers.load', 'Load')}: ${load}`}
                  </div>
                  {worker.mount_message && (
                    <div className="mt-2 text-xs text-warning">
                      {worker.mount_message}
                    </div>
                  )}
                </div>
                <PrimaryButton
                  variant="secondary"
                  value={
                    worker.status === WorkerNodeStatus.draining
                      ? t('settings.workers.resume', 'Resume scheduling')
                      : t('settings.workers.drain', 'Drain')
                  }
                  disabled={drain.isPending}
                  onClick={() =>
                    drain.mutate({
                      workerNodeId: worker.id,
                      draining: worker.status !== WorkerNodeStatus.draining,
                    })
                  }
                />
              </div>
            </div>
          );
        })}
      </div>
    </SettingsCard>
  );
}
