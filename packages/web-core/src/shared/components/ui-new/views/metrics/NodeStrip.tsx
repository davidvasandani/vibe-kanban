import { useTranslation } from 'react-i18next';
import type {
  MetricsNode,
  NodeHealth,
  NodeMetricsAvailability,
} from 'shared/types';
import { Meter } from '@vibe/ui/components/Meter';
import { WarningIcon } from '@phosphor-icons/react';
import type { NodeDiskAlert } from './diskAlerts';

import { cn } from '@/shared/lib/utils';
import {
  formatLoad,
  formatPercent,
  formatTimestamp,
  ratioPercent,
  formatBytes,
} from './format';
import { MetricsRow } from './MetricsSection';

export interface NodeStripProps {
  node: MetricsNode;
  selected: boolean;
  onSelect: (nodeId: string) => void;
  diskAlert?: NodeDiskAlert | null;
  onResolveDiskAlert?: (nodeId: string) => void;
  resolvingDiskAlert?: boolean;
  diskAlertActionDisabledReason?: string | null;
}

/** True when the node's readings are retained but no longer current. */
export function isStale(availability: NodeMetricsAvailability): boolean {
  return availability.status === 'stale';
}

/**
 * True when the node has no *current* readings, for any reason. `stale` is
 * excluded: a stale node still shows its retained readings (FR-18).
 */
export function hasNoCurrentReadings(
  availability: NodeMetricsAvailability
): boolean {
  return availability.status !== 'available';
}

/**
 * Metrics availability — "could we read this machine?".
 *
 * Deliberately separate from {@link HealthBadge}. Availability answers a
 * question about *this panel's collection path*; health answers a question
 * about the *cluster's* view of the node. They disagree routinely — a worker
 * whose lease has expired is unhealthy but perfectly readable — and collapsing
 * them would reproduce exactly the inconsistency FR-24 forbids.
 */
export function AvailabilityBadge({
  availability,
}: {
  availability: NodeMetricsAvailability;
}) {
  const { t } = useTranslation('common');

  const label = (() => {
    switch (availability.status) {
      case 'available':
        return t('metrics.availability.available', {
          defaultValue: 'Live',
        });
      case 'stale':
        return t('metrics.availability.stale', {
          defaultValue: 'Stale since {{since}}',
          since: formatTimestamp(availability.since),
        });
      case 'not_collected':
        return t('metrics.availability.notCollected', {
          defaultValue: 'Not collected yet',
        });
      case 'unsupported':
        return t('metrics.availability.unsupported', {
          defaultValue: 'Unsupported platform ({{platform}})',
          platform: availability.platform,
        });
      case 'unreachable':
        return t('metrics.availability.unreachable', {
          defaultValue: 'Unreachable: {{reason}}',
          reason: availability.reason,
        });
      case 'not_implemented':
        return t('metrics.availability.notImplemented', {
          defaultValue: 'Not supported by this node’s version',
        });
    }
  })();

  const tone =
    availability.status === 'available'
      ? 'text-normal'
      : availability.status === 'stale'
        ? 'text-brand-secondary'
        : 'text-error';

  return (
    <span
      data-testid="metrics-node-availability"
      data-status={availability.status}
      className={cn('text-sm truncate', tone)}
    >
      {label}
    </span>
  );
}

/**
 * Cluster health — "does the cluster consider this node healthy?".
 *
 * Sourced read-only from the worker row and rendered verbatim so that a node
 * cannot be healthy here and dead in Settings (FR-24).
 */
export function HealthBadge({ health }: { health: NodeHealth | null }) {
  const { t } = useTranslation('common');

  if (!health) {
    // The coordinator has no worker row to be judged by.
    return (
      <span
        data-testid="metrics-node-health"
        data-status="none"
        className="text-sm text-low truncate"
      >
        {t('metrics.health.none', { defaultValue: 'No health record' })}
      </span>
    );
  }

  const statusLabel = (() => {
    switch (health.status) {
      case 'online':
        return t('metrics.health.online', { defaultValue: 'Online' });
      case 'draining':
        return t('metrics.health.draining', { defaultValue: 'Draining' });
      case 'offline':
      default:
        return t('metrics.health.offline', { defaultValue: 'Offline' });
    }
  })();

  const suffix = health.schedulable
    ? t('metrics.health.schedulable', { defaultValue: 'schedulable' })
    : t('metrics.health.notSchedulable', { defaultValue: 'not schedulable' });

  return (
    <span
      data-testid="metrics-node-health"
      data-status={health.status}
      data-schedulable={health.schedulable ? 'true' : 'false'}
      className={cn(
        'text-sm truncate',
        health.status === 'online' ? 'text-normal' : 'text-error'
      )}
    >
      {t('metrics.health.summary', {
        defaultValue: '{{status}} · {{suffix}}',
        status: statusLabel,
        suffix,
      })}
    </span>
  );
}

/**
 * The compact per-node summary in the node list (FR-10, FR-11).
 *
 * Renders **both** health and availability, and a `null` reading as an em
 * dash rather than a zero.
 */
export function NodeStrip({
  node,
  selected,
  onSelect,
  diskAlert,
  onResolveDiskAlert,
  resolvingDiskAlert,
  diskAlertActionDisabledReason,
}: NodeStripProps) {
  const { t } = useTranslation('common');

  const stale = isStale(node.availability);
  const readings = node.latest;
  const cpuPercent = readings?.cpu.total_busy_percent ?? null;
  const memoryPercent = ratioPercent(
    readings?.memory.used_bytes,
    readings?.memory.total_bytes
  );

  const roleLabel =
    node.role === 'coordinator'
      ? t('metrics.role.coordinator', { defaultValue: 'Coordinator' })
      : t('metrics.role.worker', { defaultValue: 'Worker' });

  return (
    <div
      data-alert-severity={diskAlert?.severity}
      className={cn(
        'flex flex-col rounded-sm border border-transparent',
        diskAlert?.severity === 'warning' && 'border-brand bg-brand/5',
        diskAlert?.severity === 'critical' && 'border-error bg-error/5'
      )}
    >
      <button
        type="button"
        data-testid="metrics-node-strip"
        data-node-id={node.node_id}
        aria-pressed={selected}
        onClick={() => onSelect(node.node_id)}
        className={cn(
          'flex flex-col gap-half w-full p-half rounded-sm text-left',
          'hover:bg-panel focus:outline-none focus:ring-1 focus:ring-brand',
          selected && 'bg-panel'
        )}
      >
        <span className="flex items-baseline justify-between gap-half min-w-0">
          <span className="text-high truncate">{node.hostname}</span>
          <span className="text-sm text-low shrink-0">{roleLabel}</span>
        </span>
        <span className="flex items-center justify-between gap-half min-w-0">
          <HealthBadge health={node.health} />
          <AvailabilityBadge availability={node.availability} />
        </span>
        {stale && (
          <span
            data-testid="metrics-node-stale"
            className="text-sm text-low truncate"
          >
            {t('metrics.staleReadings', {
              defaultValue: 'Readings captured {{captured}}',
              captured: formatTimestamp(
                readings?.captured_at ?? node.last_contact_at
              ),
            })}
          </span>
        )}
        <span
          data-testid="metrics-node-readings"
          data-stale={stale ? 'true' : undefined}
          className={cn('flex flex-col gap-half', stale && 'opacity-60')}
        >
          <Meter
            label={t('metrics.cpu.title', { defaultValue: 'CPU' })}
            value={cpuPercent}
            valueText={formatPercent(cpuPercent)}
          />
          <Meter
            label={t('metrics.memory.title', { defaultValue: 'Memory' })}
            value={memoryPercent}
            valueText={formatPercent(memoryPercent)}
          />
          {/*
           * A row rather than a Meter: load average is unbounded, so there is no
           * honest denominator to fill a 0..100 bar with. Saturation is load
           * relative to core count, which the CPU panel spells out; here the
           * three windows are shown raw so a rising or falling trend is visible
           * without expanding the node.
           */}
          <MetricsRow
            label={t('metrics.cpu.loadShort', { defaultValue: 'Load' })}
            value={`${formatLoad(readings?.cpu.load_1m)} / ${formatLoad(
              readings?.cpu.load_5m
            )} / ${formatLoad(readings?.cpu.load_15m)}`}
          />
        </span>
      </button>
      {diskAlert && (
        <button
          type="button"
          data-testid="metrics-disk-alert"
          disabled={!onResolveDiskAlert || resolvingDiskAlert}
          title={diskAlertActionDisabledReason ?? undefined}
          onClick={() => onResolveDiskAlert?.(node.node_id)}
          className={cn(
            'flex items-start gap-half mx-half mb-half p-half rounded-sm text-left',
            'focus:outline-none focus:ring-1 focus:ring-brand disabled:cursor-not-allowed',
            diskAlert.severity === 'critical'
              ? 'text-error'
              : 'text-brand-secondary'
          )}
          aria-label={`${diskAlert.severity === 'critical' ? 'Critical disk' : 'Low disk'} on ${node.hostname}. ${diskAlertActionDisabledReason ?? 'Create or open remediation issue'}`}
        >
          <WarningIcon weight="fill" className="shrink-0" aria-hidden="true" />
          <span className="flex flex-col min-w-0 text-sm">
            <span className="font-medium">
              {diskAlert.severity === 'critical' ? 'Critical disk' : 'Low disk'}
            </span>
            {diskAlert.filesystems.map(({ filesystem, usedPercent }) => (
              <span key={`${filesystem.mount_point}:${filesystem.device}`}>
                {filesystem.device} · {formatBytes(filesystem.available_bytes)}{' '}
                available · {formatPercent(usedPercent)} used ·{' '}
                {filesystem.mount_point}
              </span>
            ))}
            {resolvingDiskAlert && <span>Resolving issue…</span>}
          </span>
        </button>
      )}
    </div>
  );
}
