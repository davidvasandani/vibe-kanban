import type {
  DiskAlertThresholds,
  FilesystemSample,
  MetricsNode,
} from 'shared/types';

export type DiskAlertSeverity = 'warning' | 'critical';

export interface FilesystemDiskAlert {
  filesystem: FilesystemSample;
  severity: DiskAlertSeverity;
  freePercent: number;
  usedPercent: number;
}

export interface NodeDiskAlert {
  nodeId: string;
  severity: DiskAlertSeverity;
  filesystems: FilesystemDiskAlert[];
}

const finiteNumber = (value: unknown): number | null => {
  if (value === null || value === undefined) return null;
  const number = Number(value);
  return Number.isFinite(number) && number >= 0 ? number : null;
};

export function classifyFilesystem(
  filesystem: FilesystemSample,
  thresholds: DiskAlertThresholds
): FilesystemDiskAlert | null {
  const total = finiteNumber(filesystem.total_bytes);
  const used = finiteNumber(filesystem.used_bytes);
  const available = finiteNumber(filesystem.available_bytes);
  if (
    total === null ||
    used === null ||
    available === null ||
    total <= 0 ||
    used > total ||
    available > total
  ) {
    return null;
  }

  const freePercent = (available / total) * 100;
  const usedPercent = (used / total) * 100;
  const critical =
    freePercent < Number(thresholds.critical_free_percent) ||
    available < Number(thresholds.critical_free_bytes);
  const warning =
    freePercent < Number(thresholds.warning_free_percent) ||
    available < Number(thresholds.warning_free_bytes);
  if (!critical && !warning) return null;

  return {
    filesystem,
    severity: critical ? 'critical' : 'warning',
    freePercent,
    usedPercent,
  };
}

export function classifyNode(
  node: MetricsNode,
  thresholds: DiskAlertThresholds
): NodeDiskAlert | null {
  const availability = node.availability?.status;
  if (availability !== 'available' && availability !== 'stale') return null;

  const filesystems = (node.latest?.filesystems ?? [])
    .map((filesystem) => classifyFilesystem(filesystem, thresholds))
    .map((alert) =>
      alert && availability === 'stale'
        ? { ...alert, severity: 'warning' as const }
        : alert
    )
    .filter((alert): alert is FilesystemDiskAlert => alert !== null)
    .sort((a, b) => {
      if (a.severity !== b.severity) return a.severity === 'critical' ? -1 : 1;
      return a.freePercent - b.freePercent;
    });
  if (filesystems.length === 0) return null;
  return {
    nodeId: node.node_id,
    severity: filesystems.some((item) => item.severity === 'critical')
      ? 'critical'
      : 'warning',
    filesystems,
  };
}

export function rollupDiskAlerts(
  nodes: readonly MetricsNode[],
  thresholds: DiskAlertThresholds
) {
  const alerts = nodes
    .map((node) => classifyNode(node, thresholds))
    .filter((alert): alert is NodeDiskAlert => alert !== null);
  return {
    alerts,
    affectedNodes: alerts.length,
    severity: alerts.some((alert) => alert.severity === 'critical')
      ? ('critical' as const)
      : alerts.length > 0
        ? ('warning' as const)
        : null,
  };
}
