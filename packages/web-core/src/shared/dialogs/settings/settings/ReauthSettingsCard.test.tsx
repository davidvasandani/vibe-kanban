/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  ReauthOverview,
  ReauthRun,
  ReauthRunReport,
  ReauthTargetStatus,
} from 'shared/types';
import {
  ReauthSettingsCard,
  isNewerRun,
  mergeRuns,
} from './ReauthSettingsCard';

vi.hoisted(() => {
  process.env.NODE_ENV = 'test';
});

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const selected = vi.hoisted(() => ({ client: null as unknown }));

const mocks = vi.hoisted(() => ({
  listReauthTargets: vi.fn<() => Promise<ReauthOverview>>(),
  listReauthRuns: vi.fn<() => Promise<ReauthRunReport[]>>(),
  runReauth: vi.fn<(target?: string) => Promise<ReauthRunReport[]>>(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('./SettingsHostContext', () => ({
  useSettingsMachineClient: () => selected.client,
}));

vi.mock('@vibe/ui/components/Button', () => ({
  Button: ({
    children,
    onClick,
    disabled,
  }: {
    children: React.ReactNode;
    onClick: () => void;
    disabled?: boolean;
  }) => (
    <button type="button" onClick={onClick} disabled={disabled}>
      {children}
    </button>
  ),
}));

vi.mock('./SettingsComponents', () => ({
  SettingsCard: ({
    title,
    children,
  }: {
    title: string;
    children: React.ReactNode;
  }) => (
    <section>
      <h2>{title}</h2>
      {children}
    </section>
  ),
}));

function run(overrides: Partial<ReauthRun> = {}): ReauthRun {
  return {
    started_at: '2026-09-26T12:00:00Z',
    finished_at: null,
    outcome: 'running',
    trigger: 'manual',
    message: null,
    transcript: [],
    already_running: false,
    ...overrides,
  };
}

function target(
  overrides: Partial<ReauthTargetStatus> = {}
): ReauthTargetStatus {
  return {
    id: 'aws-sso:sweetgreen',
    kind: 'aws_sso',
    label: 'AWS SSO · sweetgreen (31 profiles)',
    swept: true,
    auth_state: 'unauthenticated',
    auth_message: null,
    refused: false,
    last_run: null,
    ...overrides,
  };
}

let container: HTMLDivElement;
let root: Root;

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function render() {
  await act(async () => {
    root.render(<ReauthSettingsCard />);
  });
  await flush();
}

function button(text: string, scope: ParentNode = container) {
  const found = Array.from(scope.querySelectorAll('button')).find(
    (b) => b.textContent === text
  );
  if (!found) throw new Error(`no button ${text}`);
  return found;
}

beforeEach(() => {
  vi.useFakeTimers();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  mocks.listReauthTargets.mockReset();
  mocks.listReauthRuns.mockReset();
  mocks.runReauth.mockReset();
  selected.client = mocks;
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.useRealTimers();
});

describe('ReauthSettingsCard', () => {
  it('lists targets with state, sweep interval and on-demand marking', async () => {
    mocks.listReauthTargets.mockResolvedValue({
      sweep_interval_secs: 1800,
      targets: [
        target(),
        target({
          id: 'sgsc:dp',
          kind: 'sgsc',
          label: 'sgsc-mcp · dp',
          swept: false,
          auth_state: 'unknown',
        }),
      ],
    } as ReauthOverview);
    await render();

    expect(container.textContent).toContain(
      'AWS SSO · sweetgreen (31 profiles)'
    );
    expect(container.textContent).toContain(
      'settings.reauth.state.unauthenticated'
    );
    expect(container.textContent).toContain('settings.reauth.sweepEvery');
    const sgsc = container.querySelector(
      '[data-testid="reauth-target-sgsc:dp"]'
    )!;
    expect(sgsc.textContent).toContain('settings.reauth.onDemand');
  });

  it('runs a target, shows it running, then the settled outcome', async () => {
    mocks.listReauthTargets.mockResolvedValue({
      sweep_interval_secs: null,
      targets: [target()],
    } as ReauthOverview);
    mocks.runReauth.mockResolvedValue([
      { id: 'aws-sso:sweetgreen', run: run() },
    ]);
    await render();

    await act(async () => {
      button('settings.reauth.run').click();
    });
    await flush();
    expect(mocks.runReauth).toHaveBeenCalledWith('aws-sso:sweetgreen');
    expect(container.textContent).toContain('settings.reauth.outcome.running');
    expect(button('settings.reauth.run').disabled).toBe(true);

    mocks.listReauthRuns.mockResolvedValue([
      {
        id: 'aws-sso:sweetgreen',
        run: run({ outcome: 'succeeded', finished_at: '2026-09-26T12:01:00Z' }),
      },
    ]);
    mocks.listReauthTargets.mockResolvedValue({
      sweep_interval_secs: null,
      targets: [
        target({
          auth_state: 'authenticated',
          last_run: run({
            outcome: 'succeeded',
            finished_at: '2026-09-26T12:01:00Z',
          }),
        }),
      ],
    } as ReauthOverview);
    await act(async () => {
      vi.advanceTimersByTime(3000);
    });
    await flush();
    expect(mocks.listReauthRuns).toHaveBeenCalled();
    expect(container.textContent).toContain(
      'settings.reauth.outcome.succeeded'
    );
    expect(container.textContent).toContain(
      'settings.reauth.state.authenticated'
    );
  });

  it('shows failure messages verbatim, including line breaks', async () => {
    const message =
      'Entra rejected the 1Password password\ncorrelation: 5f1d-aaaa';
    mocks.listReauthTargets.mockResolvedValue({
      sweep_interval_secs: null,
      targets: [
        target({
          refused: true,
          last_run: run({
            outcome: 'refused',
            finished_at: '2026-09-26T12:01:00Z',
            message,
            transcript: [
              '  sign-in step: Password (https://login.microsoftonline.com/t)',
            ],
          }),
        }),
      ],
    } as ReauthOverview);
    await render();

    const paragraphs = Array.from(container.querySelectorAll('p'));
    expect(paragraphs.some((p) => p.textContent === message)).toBe(true);
    expect(container.textContent).toContain('settings.reauth.refused');
    await act(async () => {
      button('settings.reauth.showSteps').click();
    });
    expect(container.querySelector('pre')?.textContent).toContain(
      'sign-in step: Password'
    );
  });

  it('renders a load error as the server reported it', async () => {
    mocks.listReauthTargets.mockRejectedValue(new Error('VK API returned 500'));
    await render();
    expect(container.textContent).toContain('VK API returned 500');
  });
});

describe('ReauthSettingsCard host switching', () => {
  it('drops a late response from the previously selected host', async () => {
    let resolveOld: (o: ReauthOverview) => void = () => {};
    const oldHost = {
      ...mocks,
      listReauthTargets: vi.fn(
        () => new Promise<ReauthOverview>((r) => (resolveOld = r))
      ),
    };
    const newHost = {
      ...mocks,
      listReauthTargets: vi.fn(async () => ({
        sweep_interval_secs: null,
        targets: [target({ id: 'cli-tool:az', label: 'new host az' })],
      })),
    };
    selected.client = oldHost;
    await render();
    selected.client = newHost;
    await render();
    expect(container.textContent).toContain('new host az');

    await act(async () => {
      resolveOld({
        sweep_interval_secs: null,
        targets: [target({ label: 'old host aws' })],
      });
    });
    await flush();
    expect(container.textContent).toContain('new host az');
    expect(container.textContent).not.toContain('old host aws');
  });
});

describe('mergeRuns', () => {
  it('never rolls a newer run back to an older or regressed one', () => {
    const newer = run({ started_at: '2026-09-26T12:05:00Z' });
    const older = run({
      started_at: '2026-09-26T12:00:00Z',
      outcome: 'succeeded',
    });
    expect(isNewerRun(newer, older)).toBe(false);
    expect(isNewerRun(older, newer)).toBe(true);
    // The same run may only move forward, never back to running.
    const done = run({ outcome: 'failed' });
    expect(isNewerRun(run(), done)).toBe(true);
    expect(isNewerRun(done, run())).toBe(false);
    expect(isNewerRun(null, run())).toBe(true);
    // A running run's transcript grows while it runs; never shrinks.
    const oneStep = run({ transcript: ['step 1'] });
    const twoSteps = run({ transcript: ['step 1', 'step 2'] });
    expect(isNewerRun(oneStep, twoSteps)).toBe(true);
    expect(isNewerRun(twoSteps, oneStep)).toBe(false);
    const overview = {
      sweep_interval_secs: null,
      targets: [target({ last_run: newer })],
    } as ReauthOverview;
    const merged = mergeRuns(overview, [
      { id: 'aws-sso:sweetgreen', run: older },
    ]);
    expect(merged.targets[0].last_run).toBe(newer);
  });

  it('replaces only targets with a newer run', () => {
    const overview = {
      sweep_interval_secs: null,
      targets: [target(), target({ id: 'cli-tool:az', label: 'az' })],
    } as ReauthOverview;
    const merged = mergeRuns(overview, [
      { id: 'cli-tool:az', run: run({ outcome: 'failed' }) },
    ]);
    expect(merged.targets[0].last_run).toBeNull();
    expect(merged.targets[1].last_run?.outcome).toBe('failed');
  });
});
