/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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
    serverMetricsSection: 'server-metrics',
    serverAffinitySection: 'server-affinity',
    terminalSection: 'terminal',
    notesSection: 'notes',
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

vi.mock('@vibe/ui/components/CollapsibleSectionHeader', () => ({
  CollapsibleSectionHeader: ({
    title,
    children,
  }: {
    title: string;
    children: React.ReactNode;
  }) => (
    <section data-testid="drawer-section">
      <button type="button">{title}</button>
      {children}
    </section>
  ),
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
vi.mock('./ServerMetricsSectionContainer', () => ({
  ServerMetricsSectionContainer: () => null,
}));
vi.mock('./ServerAffinitySectionContainer', () => ({
  ServerAffinitySectionContainer: () => null,
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

import { RightSidebar } from './RightSidebar';

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime('2026-08-09T15:00:00Z');
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  vi.useRealTimers();
});

describe('RightSidebar deploy status', () => {
  it('renders a fixed, non-collapsible status row before drawer sections', () => {
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

    const row = container.querySelector('[data-testid="deploy-status-row"]');
    expect(row).not.toBeNull();
    expect(row?.classList).toContain('flex-none');
    expect(row?.classList).toContain('shrink-0');
    expect(row?.textContent).toContain('Deploy Status');
    expect(row?.textContent).toContain('abc1234');
    expect(row?.textContent).toContain('· 2h');
    expect(row?.querySelector('button')).toBeNull();

    const stack = row?.parentElement;
    expect(stack?.firstElementChild).toBe(row);
    expect(
      stack?.querySelectorAll('[data-testid="drawer-section"]').length
    ).toBeGreaterThan(0);
  });
});
