/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  type WorkerNode,
} from 'shared/types';
import { WorkersSettingsSection } from './WorkersSettingsSection';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  list: vi.fn<() => Promise<WorkerNode[]>>(),
  setDraining: vi.fn<(id: string, draining: boolean) => Promise<WorkerNode>>(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback: string) => fallback,
  }),
}));

vi.mock('@/shared/lib/api', () => ({
  workerNodesApi: {
    list: mocks.list,
    setDraining: mocks.setDraining,
  },
}));

vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({
    value,
    onClick,
    disabled,
  }: {
    value: string;
    onClick: () => void;
    disabled?: boolean;
  }) => (
    <button type="button" onClick={onClick} disabled={disabled}>
      {value}
    </button>
  ),
}));

vi.mock('./SettingsComponents', () => ({
  SettingsCard: ({
    title,
    description,
    children,
  }: {
    title: string;
    description: string;
    children: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      <p>{description}</p>
      {children}
    </section>
  ),
}));

function worker(overrides: Partial<WorkerNode> = {}): WorkerNode {
  return {
    id: 'worker-1',
    hostname: 'think3',
    status: WorkerNodeStatus.online,
    worker_version: '1',
    vibe_version: '1',
    capabilities: { executor_profiles: ['codex'] },
    resource_snapshot: { load_1m: 0.5, active_execution_count: 2 },
    labels: {},
    mount_status: WorkerMountStatus.healthy,
    mount_message: null,
    last_heartbeat_at: '2026-07-31T00:00:00Z',
    lease_expires_at: '2026-07-31T00:00:30Z',
    created_at: '2026-07-31T00:00:00Z',
    updated_at: '2026-07-31T00:00:00Z',
    ...overrides,
  };
}

async function renderSection() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  queryClient.setQueryData(['worker-nodes'], await mocks.list());
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(
      <QueryClientProvider client={queryClient}>
        <WorkersSettingsSection />
      </QueryClientProvider>
    );
  });
  return { container, root, queryClient };
}

describe('WorkersSettingsSection', () => {
  let root: Root | null = null;

  beforeEach(() => {
    mocks.list.mockResolvedValue([worker()]);
    mocks.setDraining.mockImplementation(async (_id, draining) =>
      worker({
        status: draining ? WorkerNodeStatus.draining : WorkerNodeStatus.offline,
      })
    );
  });

  afterEach(() => {
    act(() => root?.unmount());
    root = null;
    vi.clearAllMocks();
    document.body.innerHTML = '';
  });

  it('shows schedulability, load, activity, and drains a worker', async () => {
    const rendered = await renderSection();
    root = rendered.root;
    await act(async () => {});

    expect(rendered.container.textContent).toContain('think3');
    expect(rendered.container.textContent).toContain('Schedulable');
    expect(rendered.container.textContent).toContain('Active executions: 2');
    expect(rendered.container.textContent).toContain('Load: 0.5');

    const drain = Array.from(
      rendered.container.querySelectorAll('button')
    ).find((button) => button.textContent === 'Drain');
    expect(drain).toBeDefined();
    await act(async () => {
      drain?.click();
    });
    expect(mocks.setDraining).toHaveBeenCalledWith('worker-1', true);
  });

  it('surfaces mount diagnostics for an unschedulable worker', async () => {
    mocks.list.mockResolvedValue([
      worker({
        mount_status: WorkerMountStatus.probe_not_visible,
        mount_message: 'coordinator probe was not visible',
      }),
    ]);
    const rendered = await renderSection();
    root = rendered.root;
    await act(async () => {});

    expect(rendered.container.textContent).toContain('Not schedulable');
    expect(rendered.container.textContent).toContain(
      'coordinator probe was not visible'
    );
  });
});
