/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WakeupSummary } from 'shared/types';
import { WakeupRow } from './WakeupRow';
import { PollersHeader } from './PollersHeader';
import { workspaceWakeupsApi } from '@/shared/lib/api';

vi.mock('@/shared/lib/api', () => ({
  workspaceWakeupsApi: { cancel: vi.fn() },
}));
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

function summary(
  state = 'pending',
  pollerId: string | null = null
): WakeupSummary {
  return {
    wakeup: {
      id: 'timer',
      workspace_id: 'workspace',
      session_id: 'session',
      fire_at: '2026-10-24T22:05:00Z',
      prompt: 'Re-check service hours.\nThen run the approved work.',
      poller_id: pollerId,
      trigger_context: null,
      created_at: '2026-10-24T12:00:00Z',
    },
    delivery: {
      state,
      reason: state === 'cancelled' ? 'workspace_archived' : null,
      error: null,
    },
  } as WakeupSummary;
}

let host: HTMLDivElement;
let root: Root;
let client: QueryClient;
beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date('2026-10-24T22:04:00Z'));
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  client.clear();
  vi.useRealTimers();
  vi.clearAllMocks();
});
function render(value = summary()) {
  act(() =>
    root.render(
      <QueryClientProvider client={client}>
        <WakeupRow summary={value} />
      </QueryClientProvider>
    )
  );
}

describe('wake-up rows', () => {
  it('shows exact prompt, deadline and a live countdown', () => {
    render();
    expect(host.textContent).toContain('0h 1m 0s remaining');
    expect(host.querySelector('time')?.dateTime).toBe('2026-10-24T22:05:00Z');
    expect(
      host.querySelector('[aria-label="Wake-up prompt"]')?.textContent
    ).toBe(summary().wakeup.prompt);
    act(() => vi.advanceTimersByTime(1000));
    expect(host.textContent).toContain('0h 0m 59s remaining');
  });
  it('distinguishes completion conditions from clock timers', () => {
    render(summary('pending', 'poller'));
    expect(host.textContent).toContain('Waiting for stop command to succeed');
    expect(host.querySelector('time')).toBeNull();
  });
  it('shows cancellation reasons and removes the Cancel action', () => {
    render(summary('cancelled'));
    expect(host.textContent).toContain('workspace archived');
    expect(host.querySelector('button')).toBeNull();
  });
  it('invalidates the shared query after cancellation', async () => {
    vi.mocked(workspaceWakeupsApi.cancel).mockResolvedValue(true);
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    render();
    await act(async () => host.querySelector('button')!.click());
    expect(workspaceWakeupsApi.cancel).toHaveBeenCalledWith(
      'workspace',
      'timer'
    );
    expect(invalidate).toHaveBeenCalledWith({
      queryKey: ['workspace-wakeups', 'workspace'],
    });
    expect(host.querySelector('[role="alert"]')).toBeNull();
  });
  it('reports a claim race rather than saying the wake-up was cancelled', async () => {
    vi.mocked(workspaceWakeupsApi.cancel).mockResolvedValue(false);
    render();
    await act(async () => host.querySelector('button')!.click());
    expect(host.querySelector('[role="alert"]')?.textContent).toContain(
      'delivery has already started'
    );
  });
  it('keeps pending wake-ups visible in the collapsed header', () => {
    act(() =>
      root.render(
        <PollersHeader executionProcesses={[]} wakeups={[summary()]} />
      )
    );
    expect(host.textContent).toContain('1 pending wake-up');
  });
});
