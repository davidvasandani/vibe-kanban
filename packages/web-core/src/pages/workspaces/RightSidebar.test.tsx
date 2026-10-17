/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Workspace } from 'shared/types';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));

vi.mock('@/shared/hooks/useUserSystem', () => ({
  useUserSystem: () => ({
    appVersion: 'abc1234',
    deploymentTimestamp: '2026-08-09T13:00:00Z',
  }),
}));

vi.mock('@/shared/hooks/useWorkspaceContext', () => ({
  useWorkspaceContext: () => ({ activeWorkspaces: [] }),
}));

vi.mock('@/shared/hooks/useLogsPanel', () => ({
  useLogsPanel: () => ({
    expandTerminal: vi.fn(),
    isTerminalExpanded: false,
  }),
}));

vi.mock('@/shared/stores/useWorkspaceDiffStore', () => ({
  useDiffs: () => [],
}));

vi.mock('@/shared/stores/useUiPreferencesStore', () => ({
  PERSIST_KEYS: {
    issueSection: 'issue',
    changesSection: 'changes',
    processesSection: 'processes',
    devServerSection: 'dev-server',
    rightPanelBrowser: 'browser',
    gitPanelRepositories: 'git',
    deployStatusSection: 'deploy-status',
    serverMetricsSection: 'server-metrics',
    serverAffinitySection: 'server-affinity',
    terminalSection: 'terminal',
    notesSection: 'notes',
    pollersSection: 'pollers',
    rightPanelprocesses: 'right-processes',
    rightPanelPreview: 'right-preview',
  },
  RIGHT_MAIN_PANEL_MODES: {
    CHANGES: 'changes',
    LOGS: 'logs',
    PREVIEW: 'preview',
    BROWSER: 'browser',
  },
  usePersistedExpanded: (_key: string, defaultValue: boolean) => [defaultValue],
  useUiPreferencesStore: (selector: (state: object) => unknown) =>
    selector({ isTerminalVisible: false }),
}));

vi.mock('./IssueSectionContainer', () => ({
  IssueSectionContainer: () => null,
}));
vi.mock('./FileTreeContainer', () => ({ FileTreeContainer: () => null }));
vi.mock('./ProcessListContainer', () => ({
  ProcessListContainer: () => null,
}));
vi.mock('./PreviewControlsContainer', () => ({
  PreviewControlsContainer: () => null,
}));
vi.mock('./BrowserControlsContainer', () => ({
  BrowserControlsContainer: () => null,
}));
vi.mock('./GitPanelContainer', () => ({ GitPanelContainer: () => null }));
vi.mock('./GitBehindHeader', () => ({
  GitBehindHeader: () => <span data-testid="git-behind-header">3 behind</span>,
}));
vi.mock('./ServerMetricsSectionContainer', () => ({
  ServerMetricsSectionContainer: () => null,
}));
vi.mock('./ServerMetricsHeader', () => ({
  ServerMetricsHeader: () => (
    <span data-testid="server-metrics-header-alert">Low disk · 1 node</span>
  ),
}));
vi.mock('./ServerAffinitySectionContainer', () => ({
  ServerAffinitySectionContainer: () => <div>Affinity controls</div>,
}));
vi.mock('./WorkspaceNotesContainer', () => ({
  WorkspaceNotesContainer: () => null,
}));
vi.mock('@/shared/components/TerminalPanelContainer', () => ({
  TerminalPanelContainer: () => null,
}));

vi.mock('./serverAffinityLabel', () => ({
  getServerAffinityLabel: () => null,
}));

// The body is stubbed, but PollersHeader is deliberately NOT mocked: the point
// of these tests is that the real header derives its summary from the streamed
// processes and survives collapse.
vi.mock('./PollersSectionContainer', () => ({
  PollersSectionContainer: () => <div>Poller rows</div>,
}));

import { RightSidebar } from './RightSidebar';
import { ExecutionProcessStatus, type ExecutionProcess } from 'shared/types';

function pollerProcess(
  id: string,
  status: ExecutionProcessStatus = ExecutionProcessStatus.running
): ExecutionProcess {
  return {
    id,
    session_id: 'session-1',
    run_reason: 'backgroundhelper',
    executor_action: {
      typ: {
        type: 'ScriptRequest',
        script: 'generated',
        language: 'Bash',
        context: 'BackgroundHelper',
        working_dir: null,
        poller: { command: 'git fetch', interval_secs: 60 },
      },
      next_action: null,
    },
    status,
    exit_code: null,
    pgid: null,
    dropped: false,
    started_at: '2026-08-31T05:00:00Z',
    completed_at: null,
    created_at: '2026-08-31T05:00:00Z',
    updated_at: '2026-08-31T05:00:00Z',
  } as unknown as ExecutionProcess;
}

function pollersHeaderText(): string | undefined {
  return (
    container.querySelector<HTMLElement>(
      '[data-testid="pollers-header-summary"]'
    )?.textContent ?? undefined
  );
}

let container: HTMLDivElement;
let root: Root;

const selectedWorkspace: Workspace = {
  id: 'workspace-id',
  task_id: null,
  container_ref: null,
  branch: 'vk/test',
  setup_completed_at: null,
  created_at: '2026-08-12T00:00:00Z',
  updated_at: '2026-08-12T00:00:00Z',
  archived: false,
  pinned: false,
  name: null,
  worktree_deleted: false,
  current_pipeline_stage: null,
  speckit_feature_key: null,
  speckit_host_repo_id: null,
  creation_status: 'ready',
  creation_error: null,
};

beforeEach(() => {
  window.localStorage.clear();
  vi.useFakeTimers();
  vi.setSystemTime('2026-08-09T15:00:00Z');
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  window.localStorage.clear();
  vi.useRealTimers();
});

describe('RightSidebar deploy status', () => {
  it('renders a collapsed Deploy Status accordion before drawer sections', () => {
    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode={null}
          selectedWorkspace={undefined}
          repos={[]}
          showDeployStatus
        />
      );
    });

    const button = Array.from(container.querySelectorAll('button')).find(
      (candidate) => candidate.textContent?.includes('Deploy Status')
    );
    const section = button?.parentElement?.parentElement;

    expect(button).toBeInstanceOf(HTMLButtonElement);
    expect(button?.textContent).toContain('abc1234');
    expect(button?.textContent).toContain('· 2h');
    expect(button?.textContent).not.toContain('Refresh');
    expect(section?.classList).toContain('flex-none');
    expect(section?.textContent).not.toContain('No newer deployment detected.');
    expect(section?.parentElement?.firstElementChild).toBe(section);
  });

  it('keeps Deploy Status first when a mode-specific section is present', () => {
    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode="changes"
          selectedWorkspace={selectedWorkspace}
          repos={[]}
          showDeployStatus
        />
      );
    });

    const firstDisclosure = container.querySelector('button');
    expect(firstDisclosure?.textContent).toContain('Deploy Status');
  });

  it('refreshes from the header action without toggling the accordion', () => {
    const onDeployRefresh = vi.fn();

    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode={null}
          selectedWorkspace={undefined}
          repos={[]}
          showDeployStatus
          deployUpdateAvailable
          onDeployRefresh={onDeployRefresh}
        />
      );
    });

    const disclosure = Array.from(container.querySelectorAll('button')).find(
      (candidate) => candidate.textContent?.includes('Deploy Status')
    );
    const refresh = container.querySelector('[aria-label="Refresh"]');
    const section = disclosure?.parentElement?.parentElement;

    expect(refresh?.textContent).toContain('Refresh');
    expect(section?.textContent).not.toContain(
      'A newer deployment is available.'
    );

    act(() => {
      refresh?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onDeployRefresh).toHaveBeenCalledTimes(1);
    expect(section?.textContent).not.toContain(
      'A newer deployment is available.'
    );
  });
});

describe('RightSidebar section sizing', () => {
  it('keeps Server Affinity intrinsic while content sections fill space', () => {
    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode={null}
          selectedWorkspace={selectedWorkspace}
          repos={[]}
        />
      );
    });

    const sectionNamed = (name: string) => {
      const button = Array.from(container.querySelectorAll('button')).find(
        (candidate) => candidate.firstElementChild?.textContent === name
      );
      const section = button?.parentElement?.parentElement;
      if (
        !(button instanceof HTMLButtonElement) ||
        !(section instanceof HTMLDivElement)
      ) {
        throw new Error(`Expected ${name} section`);
      }
      return { button, section };
    };

    const affinity = sectionNamed('Server Affinity');
    act(() => affinity.button.click());

    expect(affinity.section.classList).toContain('flex-none');
    expect(affinity.section.classList).toContain('h-auto');
    expect(affinity.section.classList).not.toContain('h-full');
    expect(affinity.section.textContent).toContain('Affinity controls');

    const git = sectionNamed('Git');
    expect(git.section.classList).toContain('flex-1');
    expect(git.section.classList).toContain('min-h-0');
  });

  it('keeps branch status in the Git header when the body is collapsed', () => {
    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode={null}
          selectedWorkspace={selectedWorkspace}
          repos={[]}
        />
      );
    });

    const indicator = container.querySelector(
      '[data-testid="git-behind-header"]'
    );
    const gitButton = Array.from(container.querySelectorAll('button')).find(
      (candidate) => candidate.textContent?.includes('Git')
    );

    expect(indicator).not.toBeNull();
    expect(gitButton?.contains(indicator)).toBe(true);

    act(() => gitButton?.click());

    expect(
      container.querySelector('[data-testid="git-behind-header"]')
    ).not.toBeNull();
  });
});

describe('RightSidebar pollers section', () => {
  const renderWithPollers = (executionProcesses: ExecutionProcess[]) => {
    act(() => {
      root.render(
        <RightSidebar
          rightMainPanelMode={null}
          selectedWorkspace={selectedWorkspace}
          repos={[]}
          executionProcesses={executionProcesses}
        />
      );
    });
  };

  const pollersButton = () =>
    Array.from(container.querySelectorAll('button')).find((candidate) =>
      candidate.textContent?.includes('Pollers')
    );

  it('shows the running poller count in the collapsed header', () => {
    renderWithPollers([pollerProcess('a'), pollerProcess('b')]);

    expect(pollersHeaderText()).toBe('2');
  });

  it('shows nothing when there are no running or failed pollers', () => {
    renderWithPollers([]);

    expect(pollersButton()).toBeInstanceOf(HTMLButtonElement);
    expect(pollersHeaderText()).toBeUndefined();
  });

  it('surfaces a failed poller distinctly from the running count', () => {
    renderWithPollers([
      pollerProcess('a'),
      pollerProcess('b', ExecutionProcessStatus.failed),
    ]);

    expect(pollersHeaderText()).toBe('1 · 1 failed');
  });

  it('keeps the summary visible when the section is collapsed', () => {
    renderWithPollers([pollerProcess('a')]);

    const button = pollersButton();
    const indicator = container.querySelector(
      '[data-testid="pollers-header-summary"]'
    );
    expect(button?.contains(indicator)).toBe(true);

    act(() => button?.click());

    expect(pollersHeaderText()).toBe('1');
  });

  it('gives the expanded section the shared flex contract', () => {
    renderWithPollers([pollerProcess('a')]);

    const button = pollersButton();
    // Sections start collapsed, so open it before asserting the expanded shape.
    act(() => button?.click());

    const section = pollersButton()?.parentElement?.parentElement;
    expect(section?.classList).toContain('flex-1');
    expect(section?.classList).toContain('min-h-0');
    expect(section?.className).not.toContain('min-h-[');
  });

  it('issues no request of its own to label the collapsed header', () => {
    // Constitution XXVI: a closed section must not stay mounted or issue a
    // private request solely to label itself. The summary is derived from the
    // execution processes the layout already streams.
    const fetchSpy = vi.fn(() =>
      Promise.reject(new Error('no fetch expected'))
    );
    vi.stubGlobal('fetch', fetchSpy);

    try {
      renderWithPollers([pollerProcess('a')]);
      expect(pollersHeaderText()).toBe('1');
      expect(fetchSpy).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
