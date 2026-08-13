import { WarningIcon } from '@phosphor-icons/react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { MetricsNode } from 'shared/types';

import { clusterMetricsApi } from '@/shared/lib/api';
import { clusterMetricsKeys } from '@/shared/lib/clusterMetricsKeys';
import { useHostId } from '@/shared/providers/HostIdProvider';
import {
  DEFAULT_DISK_ALERT_THRESHOLDS,
  rollupDiskAlerts,
} from '@/shared/components/ui-new/views/metrics/diskAlerts';
import { cn } from '@/shared/lib/utils';

export function ServerMetricsHeader() {
  const { t } = useTranslation('common');
  const hostId = useHostId();
  const { data } = useQuery({
    queryKey: clusterMetricsKeys.snapshot(hostId),
    queryFn: ({ signal }) => clusterMetricsApi.snapshot(hostId, { signal }),
    refetchInterval: 30_000,
    retry: false,
  });

  if (!data) return null;
  const nodes = Object.values(data.nodes).filter(
    (node): node is MetricsNode => !!node
  );
  const rollup = rollupDiskAlerts(
    nodes,
    data.disk_alert_thresholds ?? DEFAULT_DISK_ALERT_THRESHOLDS
  );
  if (!rollup.severity) return null;

  const severity = t(
    rollup.severity === 'critical'
      ? 'metricsDiskAlerts.critical'
      : 'metricsDiskAlerts.warning'
  );
  const label = t('metricsDiskAlerts.nodeCount', {
    defaultValue: '{{severity}} · {{count}} nodes',
    severity,
    count: rollup.affectedNodes,
  });
  return (
    <span
      data-testid="server-metrics-header-alert"
      title={label}
      className={cn(
        'flex items-center gap-half min-w-0 max-w-32 truncate text-sm',
        rollup.severity === 'critical' ? 'text-error' : 'text-brand-secondary'
      )}
    >
      <WarningIcon weight="fill" className="shrink-0" aria-hidden="true" />
      <span className="truncate">{label}</span>
    </span>
  );
}
