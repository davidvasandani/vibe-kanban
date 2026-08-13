/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClusterMetricsSnapshot } from 'shared/types';

const mocks = vi.hoisted(() => ({ snapshot: vi.fn() }));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { severity?: string; count?: number }) => {
      if (key.endsWith('critical')) return 'Critical disk';
      if (key.endsWith('warning')) return 'Low disk';
      return `${options?.severity} · ${options?.count} node`;
    },
  }),
}));
vi.mock('@/shared/lib/api', () => ({
  clusterMetricsApi: { snapshot: mocks.snapshot },
}));
vi.mock('@/shared/providers/HostIdProvider', () => ({ useHostId: () => null }));

import { ServerMetricsHeader } from './ServerMetricsHeader';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('ServerMetricsHeader', () => {
  let container: HTMLDivElement;
  let root: Root;
  let queryClient: QueryClient;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false, gcTime: 0 } },
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    queryClient.clear();
    container.remove();
  });

  it('rolls critical disk state up while the accordion body is absent', async () => {
    mocks.snapshot.mockResolvedValue({
      generated_at: '2026-08-13T00:00:00Z',
      sample_interval_ms: 2000n,
      disk_alert_thresholds: {
        warning_free_percent: 10,
        warning_free_bytes: 5n * 1024n ** 3n,
        critical_free_percent: 2,
        critical_free_bytes: 1024n ** 3n,
      },
      nodes: {
        node: {
          node_id: '00000000-0000-0000-0000-000000000001',
          hostname: 'think4',
          role: 'worker',
          health: null,
          availability: { status: 'available' },
          latest: {
            sequence: 1n,
            hostname: 'think4',
            captured_at: '2026-08-13T00:00:00Z',
            interval_ms: 2000n,
            uptime_seconds: 1n,
            cpu: {},
            memory: {},
            filesystems: [
              {
                mount_point: '/',
                device: '/dev/root',
                fs_type: 'ext4',
                total_bytes: 100n * 1024n ** 3n,
                used_bytes: 99n * 1024n ** 3n + 512n * 1024n ** 2n,
                available_bytes: 512n * 1024n ** 2n,
              },
            ],
            networks: [],
            processes: [],
            degraded: [],
          },
          history: [],
          last_contact_at: '2026-08-13T00:00:00Z',
        },
      },
    } satisfies ClusterMetricsSnapshot);

    await act(async () => {
      root.render(
        <QueryClientProvider client={queryClient}>
          <ServerMetricsHeader />
        </QueryClientProvider>
      );
    });
    for (let index = 0; index < 5; index += 1) {
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    }

    const alert = container.querySelector(
      '[data-testid="server-metrics-header-alert"]'
    );
    expect(alert?.textContent).toBe('Critical disk · 1 node');
  });
});
