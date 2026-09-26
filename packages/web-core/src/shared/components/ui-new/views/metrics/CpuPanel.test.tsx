/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CpuSample } from 'shared/types';

import { CpuPanel } from './CpuPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) =>
      typeof options?.defaultValue === 'string' ? options.defaultValue : key,
  }),
}));

(
  globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const cpu = (overrides: Partial<CpuSample> = {}): CpuSample => ({
  model: null,
  core_count: 6,
  total_busy_percent: 40,
  per_core_busy: null,
  load_1m: 21.1,
  load_5m: 20.9,
  load_15m: 20.8,
  frequency_mhz: null,
  temperature_celsius: null,
  procs_blocked: 2,
  io_pressure_some_avg60: 2.17,
  io_pressure_full_avg60: 0.86,
  ...overrides,
});

describe('CpuPanel pressure rows', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  const render = (sample: CpuSample) =>
    act(() =>
      root.render(
        <CpuPanel cpu={sample} history={[]} expanded onToggle={() => {}} />
      )
    );

  const row = (prefix: string) =>
    container.querySelector<HTMLElement>(`[aria-label^="${prefix}"]`);

  it('shows blocked tasks and io pressure without flagging a healthy node', () => {
    render(cpu());
    const blocked = row('Blocked tasks (D state)');
    expect(blocked?.getAttribute('aria-label')).toBe(
      'Blocked tasks (D state): 2'
    );
    expect(blocked?.querySelector('.text-error')).toBeNull();
    expect(row('I/O pressure')?.getAttribute('aria-label')).toBe(
      'I/O pressure 60s (some / full): 2.2% / 0.9%'
    );
  });

  it('flags blocked tasks at or above the core count', () => {
    render(cpu({ procs_blocked: 9 }));
    expect(
      row('Blocked tasks (D state)')?.querySelector('.text-error')
    ).not.toBeNull();
  });

  it('renders absent readings as no reading, not zero', () => {
    render(
      cpu({
        procs_blocked: null,
        io_pressure_some_avg60: null,
        io_pressure_full_avg60: null,
      })
    );
    expect(row('Blocked tasks (D state)')?.textContent).not.toContain('0');
    expect(
      row('Blocked tasks (D state)')?.querySelector('.text-error')
    ).toBeNull();
  });
});
