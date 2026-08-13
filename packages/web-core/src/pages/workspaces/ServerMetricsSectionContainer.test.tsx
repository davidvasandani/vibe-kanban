/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ClusterMetricsSnapshot,
  HostSample,
  MetricsNode,
  NodeMetricsAvailability,
} from 'shared/types';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  snapshot: vi.fn<() => Promise<ClusterMetricsSnapshot>>(),
  resolveLowDiskIssue: vi.fn(),
  openWebSocket: vi.fn(),
  goToProjectIssue: vi.fn(),
  awaitTxId: vi.fn(),
}));

vi.mock('@/shared/hooks/useAppNavigation', () => ({
  useAppNavigation: () => ({ goToProjectIssue: mocks.goToProjectIssue }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      const template =
        typeof options?.defaultValue === 'string' ? options.defaultValue : key;
      return template.replace(/{{(\w+)}}/g, (_match, name: string) =>
        String(options?.[name] ?? '')
      );
    },
  }),
}));

vi.mock('@/shared/lib/api', () => ({
  clusterMetricsApi: {
    snapshot: mocks.snapshot,
    resolveLowDiskIssue: mocks.resolveLowDiskIssue,
  },
}));

vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => null,
  getCurrentHostId: () => null,
}));

vi.mock('@/shared/lib/localApiTransport', () => ({
  openLocalApiWebSocket: mocks.openWebSocket,
}));

vi.mock('@/shared/lib/electric/collections', () => ({
  createShapeCollection: () => ({
    utils: { awaitTxId: mocks.awaitTxId },
  }),
}));

import { ServerMetricsSectionContainer } from './ServerMetricsSectionContainer';

/** A socket that connects but never delivers `Ready`, so REST stays in play. */
class SilentWebSocket {
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: unknown) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: unknown) => void) | null = null;
  close = vi.fn();
}

function cpuSample(overrides: Partial<HostSample['cpu']> = {}) {
  return {
    model: 'Test CPU',
    core_count: 4,
    total_busy_percent: 12.5,
    per_core_busy: [
      { core: 0, busy_percent: 10 },
      { core: 1, busy_percent: 15 },
      { core: 2, busy_percent: 12 },
      { core: 3, busy_percent: 13 },
    ],
    load_1m: 0.5,
    load_5m: 0.4,
    load_15m: 0.3,
    frequency_mhz: 3200,
    temperature_celsius: 44,
    ...overrides,
  };
}

function memorySample(overrides: Partial<HostSample['memory']> = {}) {
  return {
    total_bytes: 16n * 1024n * 1024n * 1024n,
    available_bytes: 8n * 1024n * 1024n * 1024n,
    used_bytes: 8n * 1024n * 1024n * 1024n,
    cached_bytes: 2n * 1024n * 1024n * 1024n,
    swap_total_bytes: 0n,
    swap_used_bytes: 0n,
    ...overrides,
  };
}

function hostSample(overrides: Partial<HostSample> = {}): HostSample {
  return {
    sequence: 1n,
    hostname: 'think3',
    captured_at: '2026-08-03T10:00:00Z',
    interval_ms: 2000n,
    uptime_seconds: 3600n,
    cpu: cpuSample(),
    memory: memorySample(),
    filesystems: [],
    networks: [],
    processes: [],
    degraded: [],
    ...overrides,
  };
}

function node(overrides: Partial<MetricsNode> = {}): MetricsNode {
  const latest = overrides.latest ?? hostSample();
  return {
    node_id: '00000000-0000-0000-0000-000000000001',
    hostname: 'think3',
    role: 'coordinator',
    health: null,
    availability: { status: 'available' },
    latest,
    history: latest ? [latest] : [],
    last_contact_at: '2026-08-03T10:00:00Z',
    ...overrides,
  };
}

function snapshotOf(...nodes: MetricsNode[]): ClusterMetricsSnapshot {
  return {
    nodes: Object.fromEntries(nodes.map((n) => [n.node_id, n])),
    generated_at: '2026-08-03T10:00:01Z',
    sample_interval_ms: 2000n,
    disk_alert_thresholds: {
      warning_free_percent: 10,
      warning_free_bytes: 5n * 1024n ** 3n,
      critical_free_percent: 2,
      critical_free_bytes: 1024n ** 3n,
    },
  };
}

let container: HTMLDivElement;
let root: Root;
let queryClient: QueryClient;

async function renderContainer(
  props: { expanded?: boolean; projectId?: string | null } = {}
) {
  await act(async () => {
    root.render(
      <QueryClientProvider client={queryClient}>
        <ServerMetricsSectionContainer {...props} />
      </QueryClientProvider>
    );
  });
  // Let the REST fallback settle: the query resolves over several microtask
  // turns before React commits its data.
  for (let i = 0; i < 5; i += 1) {
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  }
}

beforeEach(() => {
  mocks.snapshot.mockReset();
  mocks.openWebSocket.mockReset();
  mocks.resolveLowDiskIssue.mockReset();
  mocks.goToProjectIssue.mockReset();
  mocks.awaitTxId.mockReset();
  mocks.awaitTxId.mockResolvedValue(undefined);
  mocks.openWebSocket.mockImplementation(() =>
    Promise.resolve(new SilentWebSocket())
  );
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  queryClient.clear();
  vi.restoreAllMocks();
});

describe('ServerMetricsSectionContainer', () => {
  it('shows concrete low-disk facts and opens the resolved issue', async () => {
    const total = 100n * 1024n ** 3n;
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node({
          latest: hostSample({
            filesystems: [
              {
                mount_point: '/',
                device: '/dev/mapper/pool-root',
                fs_type: 'ext4',
                total_bytes: total,
                used_bytes: total - 512n * 1024n ** 2n,
                available_bytes: 512n * 1024n ** 2n,
              },
            ],
          }),
        })
      )
    );
    mocks.resolveLowDiskIssue.mockResolvedValue({
      issue: { id: 'issue-id' },
      txid: 1n,
      created: true,
    });

    await renderContainer({ projectId: 'project-id' });

    const alert = container.querySelector<HTMLButtonElement>(
      '[data-testid="metrics-disk-alert"]'
    );
    expect(alert?.textContent).toContain('Critical disk');
    expect(alert?.textContent).toContain('/dev/mapper/pool-root');
    expect(alert?.textContent).toContain('512 MiB available');
    expect(alert?.textContent).toContain('99.5% used');
    expect(alert?.textContent).toContain('/');

    await act(async () => alert?.click());
    expect(mocks.resolveLowDiskIssue).toHaveBeenCalledWith(
      expect.objectContaining({
        project_id: 'project-id',
        node_id: '00000000-0000-0000-0000-000000000001',
      }),
      null
    );
    expect(mocks.awaitTxId).toHaveBeenCalledWith(1, 10_000);
    expect(mocks.goToProjectIssue).toHaveBeenCalledWith(
      'project-id',
      'issue-id'
    );
  });

  it('renders one entry per node', async () => {
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node(),
        node({
          node_id: '00000000-0000-0000-0000-000000000002',
          hostname: 'think4',
          role: 'worker',
        }),
        node({
          node_id: '00000000-0000-0000-0000-000000000003',
          hostname: 'think5',
          role: 'worker',
        })
      )
    );

    await renderContainer();

    const entries = container.querySelectorAll('[data-testid="metrics-node"]');
    expect(entries).toHaveLength(3);
    // Coordinator first, then workers by hostname.
    expect(
      Array.from(entries).map(
        (entry) =>
          entry.querySelector('[data-testid="metrics-node-strip"]')
            ?.textContent ?? ''
      )[0]
    ).toContain('think3');
  });

  it.each<[string, NodeMetricsAvailability, string]>([
    [
      'unreachable',
      { status: 'unreachable', reason: 'lease expired' },
      'Unreachable: lease expired',
    ],
    [
      'unsupported',
      { status: 'unsupported', platform: 'darwin' },
      'Unsupported platform (darwin)',
    ],
    [
      'not_implemented',
      { status: 'not_implemented' },
      'Not supported by this node’s version',
    ],
    ['not_collected', { status: 'not_collected' }, 'Not collected yet'],
  ])(
    'renders the %s status as its own message, never as zeros',
    async (status, availability, message) => {
      mocks.snapshot.mockResolvedValue(
        snapshotOf(node({ availability, latest: null, history: [] }))
      );

      await renderContainer();

      const badge = container.querySelector(
        '[data-testid="metrics-node-availability"]'
      );
      expect(badge?.getAttribute('data-status')).toBe(status);
      expect(badge?.textContent).toBe(message);

      // Both readings are absent, so both are em dashes and neither is `0`.
      const readings = container.querySelector(
        '[data-testid="metrics-node-readings"]'
      );
      expect(
        readings?.querySelectorAll('[data-testid="meter-no-reading"]')
      ).toHaveLength(2);
      expect(readings?.textContent).toContain('—');
      expect(readings?.textContent).not.toContain('0');
      expect(
        readings?.querySelectorAll('[data-testid="meter-fill"]')
      ).toHaveLength(0);
    }
  );

  it('renders a stale node with retained readings, dimmed and timestamped', async () => {
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node({
          availability: { status: 'stale', since: '2026-08-03T09:59:00Z' },
        })
      )
    );

    await renderContainer();

    const badge = container.querySelector(
      '[data-testid="metrics-node-availability"]'
    );
    expect(badge?.getAttribute('data-status')).toBe('stale');
    expect(badge?.textContent).toContain('Stale since');

    expect(
      container.querySelector('[data-testid="metrics-node-stale"]')?.textContent
    ).toContain('Readings captured');

    const readings = container.querySelector(
      '[data-testid="metrics-node-readings"]'
    );
    // Retained, so still rendered — but de-emphasised.
    expect(readings?.getAttribute('data-stale')).toBe('true');
    expect(readings?.className).toContain('opacity-60');
    expect(readings?.textContent).toContain('12.5%');
  });

  it('renders a null reading as an em dash rather than 0', async () => {
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node({
          latest: hostSample({
            cpu: cpuSample({
              total_busy_percent: null,
              // Load is absent too, so the whole readings block can be
              // asserted to contain no digit at all — the strongest form of
              // "a missing reading is never a zero".
              load_1m: null,
              load_5m: null,
              load_15m: null,
            }),
            memory: memorySample({ total_bytes: null, used_bytes: null }),
          }),
        })
      )
    );

    await renderContainer();

    const readings = container.querySelector(
      '[data-testid="metrics-node-readings"]'
    );
    expect(
      readings?.querySelectorAll('[data-testid="meter-no-reading"]')
    ).toHaveLength(2);
    expect(readings?.textContent).toBe('CPU—Memory—Load— / — / —');
    // The invariant: a missing reading is never rendered as a zero. An absent
    // load must not read as `0.00`, which would say "idle" rather than
    // "unknown".
    expect(readings?.textContent).not.toContain('0');
    expect(
      readings?.querySelectorAll('[data-testid="meter-fill"]')
    ).toHaveLength(0);
  });

  it('shows all three load averages in the node summary', async () => {
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node({
          latest: hostSample({
            cpu: cpuSample({ load_1m: 3.61, load_5m: 3.17, load_15m: 3.19 }),
          }),
        })
      )
    );

    await renderContainer();

    const readings = container.querySelector(
      '[data-testid="metrics-node-readings"]'
    );
    // Two decimal places, all three windows, so a rising or falling trend is
    // legible without expanding the node.
    expect(readings?.textContent).toContain('3.61 / 3.17 / 3.19');
  });

  it('expands a node into its panels, rendering rate-derived nulls as em dashes', async () => {
    mocks.snapshot.mockResolvedValue(
      snapshotOf(
        node({
          latest: hostSample({
            networks: [
              {
                interface: 'eth0',
                rx_bytes_total: 1024n,
                tx_bytes_total: 2048n,
                // No predecessor yet, so there is no rate to report.
                rx_bytes_per_second: null,
                tx_bytes_per_second: null,
              },
            ],
          }),
        })
      )
    );

    await renderContainer();

    expect(
      container.querySelector('[data-testid="metrics-node-detail"]')
    ).toBeNull();

    const strip = container.querySelector<HTMLButtonElement>(
      '[data-testid="metrics-node-strip"]'
    );
    await act(async () => {
      strip?.click();
    });

    expect(
      container.querySelector('[data-testid="metrics-node-detail"]')
    ).not.toBeNull();

    // The network panel starts collapsed; open it.
    const networkHeader = container.querySelector<HTMLButtonElement>(
      '[data-testid="metrics-section-network"] button'
    );
    await act(async () => {
      networkHeader?.click();
    });

    const iface = container.querySelector('[data-testid="metrics-interface"]');
    expect(iface?.textContent).toContain('— ↓ / — ↑');
  });

  it('opens no socket and issues no request while the section is collapsed', async () => {
    mocks.snapshot.mockResolvedValue(snapshotOf(node()));

    await renderContainer({ expanded: false });

    expect(mocks.openWebSocket).not.toHaveBeenCalled();
    expect(mocks.snapshot).not.toHaveBeenCalled();

    // Expanding is what opens it.
    await renderContainer({ expanded: true });
    expect(mocks.openWebSocket).toHaveBeenCalledTimes(1);
    expect(mocks.openWebSocket).toHaveBeenCalledWith('/api/cluster/metrics/ws');
  });

  it('keeps rendering the other nodes when one node is malformed', async () => {
    const malformed = {
      ...node({
        node_id: '00000000-0000-0000-0000-000000000009',
        hostname: 'broken',
        role: 'worker',
      }),
      // A node from a version that did not send an availability at all.
      availability: undefined as unknown as NodeMetricsAvailability,
    };

    mocks.snapshot.mockResolvedValue(
      snapshotOf(node({ hostname: 'think3' }), malformed)
    );

    // The boundary logs the crash; keep the suite output readable.
    const consoleError = vi
      .spyOn(console, 'error')
      .mockImplementation(() => undefined);

    await renderContainer();

    consoleError.mockRestore();

    expect(
      container.querySelectorAll('[data-testid="metrics-node-error"]')
    ).toHaveLength(1);
    // The healthy node is still there, with its readings.
    const strips = container.querySelectorAll(
      '[data-testid="metrics-node-strip"]'
    );
    expect(strips).toHaveLength(1);
    expect(strips[0].textContent).toContain('think3');
    expect(container.textContent).toContain('12.5%');
  });
});
