import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { IssueSectionContainer } from './IssueSectionContainer';
import { FileTreeContainer } from './FileTreeContainer';
import { ProcessListContainer } from './ProcessListContainer';
import { PreviewControlsContainer } from './PreviewControlsContainer';
import { BrowserControlsContainer } from './BrowserControlsContainer';
import { GitPanelContainer } from './GitPanelContainer';
import { ServerMetricsSectionContainer } from './ServerMetricsSectionContainer';
import { ServerAffinitySectionContainer } from './ServerAffinitySectionContainer';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { TerminalPanelContainer } from '@/shared/components/TerminalPanelContainer';
import { WorkspaceNotesContainer } from './WorkspaceNotesContainer';
import { useDiffs } from '@/shared/stores/useWorkspaceDiffStore';
import { ArrowsOutSimpleIcon } from '@phosphor-icons/react';
import { useLogsPanel } from '@/shared/hooks/useLogsPanel';
import type { RepoWithTargetBranch, Workspace } from 'shared/types';
import {
  PERSIST_KEYS,
  PersistKey,
  RIGHT_MAIN_PANEL_MODES,
  type RightMainPanelMode,
  usePersistedExpanded,
  useUiPreferencesStore,
} from '@/shared/stores/useUiPreferencesStore';
import {
  CollapsibleSectionHeader,
  type SectionAction,
} from '@vibe/ui/components/CollapsibleSectionHeader';

type SectionDef = {
  title: string;
  persistKey: PersistKey;
  visible: boolean;
  expanded: boolean;
  collapsible?: boolean;
  content: React.ReactNode;
  actions: SectionAction[];
  headerExtra?: React.ReactNode;
};

export interface RightSidebarProps {
  rightMainPanelMode: RightMainPanelMode | null;
  selectedWorkspace: Workspace | undefined;
  repos: RepoWithTargetBranch[];
  linkedIssueForWorkspace?: { remoteProjectId: string; issueId: string } | null;
}

export const RightSidebar = memo(function RightSidebar({
  rightMainPanelMode,
  selectedWorkspace,
  repos,
  linkedIssueForWorkspace,
}: RightSidebarProps) {
  const { t } = useTranslation(['tasks', 'common']);
  const diffs = useDiffs();
  const isTerminalVisible = useUiPreferencesStore((s) => s.isTerminalVisible);
  const { expandTerminal, isTerminalExpanded } = useLogsPanel();
  const { activeWorkspaces } = useWorkspaceContext();
  const selectedWorkspaceSummary = activeWorkspaces.find(
    (workspace) => workspace.id === selectedWorkspace?.id
  );

  const [changesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.changesSection,
    true
  );
  const [processesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.processesSection,
    true
  );
  const [devServerExpanded] = usePersistedExpanded(
    PERSIST_KEYS.devServerSection,
    true
  );
  const [browserExpanded] = usePersistedExpanded(
    PERSIST_KEYS.rightPanelBrowser,
    true
  );
  const [gitExpanded] = usePersistedExpanded(
    PERSIST_KEYS.gitPanelRepositories,
    true
  );
  const [serverMetricsExpanded] = usePersistedExpanded(
    PERSIST_KEYS.serverMetricsSection,
    false
  );
  const [serverAffinityExpanded] = usePersistedExpanded(
    PERSIST_KEYS.serverAffinitySection,
    false
  );
  const [terminalExpanded] = usePersistedExpanded(
    PERSIST_KEYS.terminalSection,
    false
  );
  const [notesExpanded] = usePersistedExpanded(
    PERSIST_KEYS.notesSection,
    false
  );

  const hasUpperContent =
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES ||
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS ||
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.PREVIEW ||
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.BROWSER;

  const upperExpanded = (() => {
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES)
      return changesExpanded;
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS)
      return processesExpanded;
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.PREVIEW)
      return devServerExpanded;
    if (rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.BROWSER)
      return browserExpanded;
    return false;
  })();

  const sections: SectionDef[] = useMemo(() => {
    const result: SectionDef[] = [
      {
        title: 'Issue',
        persistKey: PERSIST_KEYS.issueSection,
        visible: !!linkedIssueForWorkspace,
        expanded: true,
        collapsible: false,
        content: (
          <IssueSectionContainer
            projectId={linkedIssueForWorkspace?.remoteProjectId}
          />
        ),
        actions: [],
      },
      {
        title: 'Git',
        persistKey: PERSIST_KEYS.gitPanelRepositories,
        visible: true,
        expanded: gitExpanded,
        content: (
          <GitPanelContainer
            selectedWorkspace={selectedWorkspace}
            repos={repos}
          />
        ),
        actions: [],
      },
      {
        title: t('common:sections.serverAffinity', {
          defaultValue: 'Server Affinity',
        }),
        persistKey: PERSIST_KEYS.serverAffinitySection,
        visible: !!selectedWorkspace,
        expanded: serverAffinityExpanded,
        headerExtra: selectedWorkspaceSummary?.serverAffinity ? (
          <span className="max-w-28 truncate text-sm text-low">
            {selectedWorkspaceSummary.serverAffinity.worker_hostname ??
              selectedWorkspaceSummary.serverAffinity
                .requested_worker_hostname ??
              t(
                `common:workspaces.serverAffinity.${selectedWorkspaceSummary.serverAffinity.kind}`
              )}
          </span>
        ) : null,
        content: selectedWorkspace ? (
          <ServerAffinitySectionContainer
            workspaceId={selectedWorkspace.id}
            isRunning={selectedWorkspaceSummary?.isRunning ?? false}
          />
        ) : null,
        actions: [],
      },
      {
        // The section body is unmounted while collapsed, which is what keeps
        // a closed section from holding the metrics socket open.
        title: t('common:sections.serverMetrics', {
          defaultValue: 'Server Metrics',
        }),
        persistKey: PERSIST_KEYS.serverMetricsSection,
        visible: true,
        expanded: serverMetricsExpanded,
        content: <ServerMetricsSectionContainer />,
        actions: [],
      },
      {
        title: 'Terminal',
        persistKey: PERSIST_KEYS.terminalSection,
        visible: isTerminalVisible && !isTerminalExpanded,
        expanded: terminalExpanded,
        content: <TerminalPanelContainer />,
        actions: [{ icon: ArrowsOutSimpleIcon, onClick: expandTerminal }],
      },
      {
        title: t('common:sections.notes'),
        persistKey: PERSIST_KEYS.notesSection,
        visible: true,
        expanded: notesExpanded,
        content: <WorkspaceNotesContainer />,
        actions: [],
      },
    ];

    switch (rightMainPanelMode) {
      case RIGHT_MAIN_PANEL_MODES.CHANGES:
        if (selectedWorkspace) {
          result.unshift({
            title: 'Changes',
            persistKey: PERSIST_KEYS.changesSection,
            visible: hasUpperContent,
            expanded: upperExpanded,
            content: (
              <FileTreeContainer
                key={selectedWorkspace.id}
                workspaceId={selectedWorkspace.id}
                diffs={diffs}
                className=""
              />
            ),
            actions: [],
          });
        }
        break;
      case RIGHT_MAIN_PANEL_MODES.LOGS:
        result.unshift({
          title: 'Logs',
          persistKey: PERSIST_KEYS.rightPanelprocesses,
          visible: hasUpperContent,
          expanded: upperExpanded,
          content: <ProcessListContainer />,
          actions: [],
        });
        break;
      case RIGHT_MAIN_PANEL_MODES.PREVIEW:
        if (selectedWorkspace) {
          result.unshift({
            title: 'Preview',
            persistKey: PERSIST_KEYS.rightPanelPreview,
            visible: hasUpperContent,
            expanded: upperExpanded,
            content: (
              <PreviewControlsContainer
                workspaceId={selectedWorkspace.id}
                className=""
              />
            ),
            actions: [],
          });
        }
        break;
      case RIGHT_MAIN_PANEL_MODES.BROWSER:
        if (selectedWorkspace) {
          result.unshift({
            title: 'Browser',
            persistKey: PERSIST_KEYS.rightPanelBrowser,
            visible: hasUpperContent,
            expanded: upperExpanded,
            content: (
              <BrowserControlsContainer
                workspaceId={selectedWorkspace.id}
                className=""
              />
            ),
            actions: [],
          });
        }
        break;
      case null:
        break;
    }

    return result;
  }, [
    rightMainPanelMode,
    selectedWorkspace,
    repos,
    diffs,
    gitExpanded,
    serverMetricsExpanded,
    serverAffinityExpanded,
    selectedWorkspaceSummary?.isRunning,
    selectedWorkspaceSummary?.serverAffinity,
    terminalExpanded,
    notesExpanded,
    changesExpanded,
    processesExpanded,
    devServerExpanded,
    browserExpanded,
    isTerminalVisible,
    isTerminalExpanded,
    hasUpperContent,
    upperExpanded,
    expandTerminal,
    t,
  ]);

  return (
    <div className="h-full border-l bg-secondary overflow-y-auto">
      <div className="divide-y border-b">
        {sections
          .filter((section) => section.visible)
          .map((section) => (
            <div
              key={section.persistKey}
              className="max-h-[max(50vh,400px)] flex flex-col overflow-hidden"
            >
              <CollapsibleSectionHeader
                title={section.title}
                persistKey={section.persistKey}
                defaultExpanded={section.expanded}
                collapsible={section.collapsible ?? true}
                actions={section.actions}
                headerExtra={section.headerExtra}
              >
                <div
                  className={`flex flex-1 border-t w-full overflow-auto ${
                    (section.collapsible ?? true)
                      ? 'min-h-[200px]'
                      : 'min-h-[1px]'
                  }`}
                >
                  {section.content}
                </div>
              </CollapsibleSectionHeader>
            </div>
          ))}
      </div>
    </div>
  );
});
