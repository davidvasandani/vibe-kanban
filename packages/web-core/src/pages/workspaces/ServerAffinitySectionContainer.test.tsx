/* @vitest-environment jsdom */
import React, { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  WorkerMountStatus,
  WorkerNodeStatus,
  WorkspaceAffinityUpdateOutcome,
  WorkspacePlacementState,
} from 'shared/types';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  getPlacement: vi.fn(),
  listWorkers: vi.fn(),
  updateAffinity: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'workspaces.serverAffinity.current': 'Current server',
        'workspaces.serverAffinity.runOn': 'Run on',
        'workspaces.serverAffinity.automatic': 'Automatic placement',
        'workspaces.serverAffinity.coordinator': 'Coordinator',
        'workspaces.serverAffinity.unavailable': 'Unavailable',
        'workspaces.serverAffinity.updated': 'Server affinity updated',
      })[key] ?? key,
  }),
}));

vi.mock('@vibe/ui/components/Select', () => ({
  Select: ({
    value,
    onValueChange,
    disabled,
    children,
  }: {
    value: string;
    onValueChange: (value: string) => void;
    disabled?: boolean;
    children: ReactNode;
  }) => (
    <select
      aria-label="Run on"
      value={value}
      disabled={disabled}
      onChange={(event) => onValueChange(event.target.value)}
    >
      {children}
    </select>
  ),
  SelectTrigger: () => null,
  SelectValue: () => null,
  SelectContent: ({ children }: { children: ReactNode }) => <>{children}</>,
  SelectItem: ({
    value,
    disabled,
    children,
  }: {
    value: string;
    disabled?: boolean;
    children: ReactNode;
  }) => (
    <option value={value} disabled={disabled}>
      {children}
    </option>
  ),
}));

vi.mock('@vibe/ui/components/ConfirmDialog', () => ({
  ConfirmDialog: { show: vi.fn() },
}));

vi.mock('sonner', () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

vi.mock('@/shared/lib/api', () => ({
  ApiError: class ApiError extends Error {},
  workspacesApi: {
    getPlacement: mocks.getPlacement,
    updateAffinity: mocks.updateAffinity,
  },
  workerNodesApi: { list: mocks.listWorkers },
}));

vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => null,
}));

vi.mock('@/shared/hooks/useExecutionProcessesContext', () => ({
  useExecutionProcessesContext: () => ({ executionProcessesAll: [] }),
}));

import { ServerAffinitySectionContainer } from './ServerAffinitySectionContainer';

const workspaceId = '00000000-0000-0000-0000-000000000001';
const workerId = '00000000-0000-0000-0000-000000000002';
let container: HTMLDivElement;
let root: Root;
let queryClient: QueryClient;

async function renderContainer() {
  await act(async () => {
    root.render(
      <QueryClientProvider client={queryClient}>
        <ServerAffinitySectionContainer
          workspaceId={workspaceId}
          isRunning={false}
        />
      </QueryClientProvider>
    );
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  mocks.getPlacement.mockResolvedValue({
    workspace_id: workspaceId,
    worker_node_id: workerId,
    placement_state: WorkspacePlacementState.ready,
    placed_at: '2026-08-06T00:00:00Z',
    placement_reason: 'automatic worker affinity update',
    requested_worker_node_id: null,
    placement_constraints: null,
  });
  mocks.listWorkers.mockResolvedValue([
    {
      id: workerId,
      hostname: 'think3',
      status: WorkerNodeStatus.online,
      mount_status: WorkerMountStatus.healthy,
      lease_expires_at: '2099-01-01T00:00:00Z',
      capabilities: {},
    },
  ]);
  mocks.updateAffinity.mockResolvedValue({
    placement: {
      workspace_id: workspaceId,
      worker_node_id: null,
      placement_state: WorkspacePlacementState.local,
      placed_at: '2026-08-06T00:00:01Z',
      placement_reason: 'coordinator affinity update',
      requested_worker_node_id: null,
      placement_constraints: null,
    },
    outcome: WorkspaceAffinityUpdateOutcome.updated,
    stopped_execution_id: null,
    started_execution: null,
    message: null,
  });
});

afterEach(() => {
  act(() => root.unmount());
  queryClient.clear();
  container.remove();
  vi.clearAllMocks();
});

describe('ServerAffinitySectionContainer', () => {
  it('lists Coordinator after Automatic and submits explicit coordinator intent', async () => {
    await renderContainer();

    const select = container.querySelector('select[aria-label="Run on"]');
    if (!(select instanceof HTMLSelectElement)) {
      throw new Error('Expected the Run on selector');
    }
    expect(
      Array.from(select.options).map((option) => option.textContent)
    ).toEqual(['Automatic placement', 'Coordinator', 'think3']);

    await act(async () => {
      select.value = 'coordinator';
      select.dispatchEvent(new Event('change', { bubbles: true }));
    });

    expect(mocks.updateAffinity).toHaveBeenCalledWith(workspaceId, {
      run_on_coordinator: true,
      requested_worker_node_id: null,
      restart_running: false,
      operation_id: null,
    });
  });
});
