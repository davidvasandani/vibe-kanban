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
import { getServerAffinityLabel } from './serverAffinityLabel';
import { DeployStatus } from '@vibe/ui/components/DeployStatus';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { GitBehindHeader } from './GitBehindHeader';
import { ServerMetricsHeader } from './ServerMetricsHeader';

type SectionDef = {
  title: string;
  persistKey: PersistKey;
  visible: boolean;
  expanded: boolean;
  fillAvailableSpace: boolean;
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
  showDeployStatus?: boolean;
}

export const RightSidebar = memo(function RightSidebar({
  rightMainPanelMode,
  selectedWorkspace,
  repos,
  linkedIssueForWorkspace,
  showDeployStatus = false,
}: RightSidebarProps) {
  const { t } = useTranslation(['tasks', 'common']);
  const { appVersion, deploymentTimestamp } = useUserSystem();
  const diffs = useDiffs();
  const isTerminalVisible = useUiPreferencesStore((s) => s.isTerminalVisible);
  const { expandTerminal, isTerminalExpanded } = useLogsPanel();
  const { activeWorkspaces } = useWorkspaceContext();
  const selectedWorkspaceSummary = activeWorkspaces.find(
    (workspace) => workspace.id === selectedWorkspace?.id
  );
  const serverAffinityLabel = getServerAffinityLabel(
    selectedWorkspaceSummary?.serverAffinity,
    (kind) => t(`common:workspaces.serverAffinity.${kind}`)
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
        fillAvailableSpace: true,
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
        fillAvailableSpace: true,
        headerExtra: (
          <GitBehindHeader workspaceId={selectedWorkspace?.id} repos={repos} />
        ),
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
        fillAvailableSpace: false,
        headerExtra: serverAffinityLabel ? (
          <span
            className="min-w-0 max-w-28 truncate text-sm text-low"
            title={serverAffinityLabel}
          >
            {serverAffinityLabel}
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
        fillAvailableSpace: true,
        headerExtra: <ServerMetricsHeader />,
        content: (
          <ServerMetricsSectionContainer
            projectId={linkedIssueForWorkspace?.remoteProjectId}
          />
        ),
        actions: [],
      },
      {
        title: 'Terminal',
        persistKey: PERSIST_KEYS.terminalSection,
        visible: isTerminalVisible && !isTerminalExpanded,
        expanded: terminalExpanded,
        fillAvailableSpace: true,
        content: <TerminalPanelContainer />,
        actions: [{ icon: ArrowsOutSimpleIcon, onClick: expandTerminal }],
      },
      {
        title: t('common:sections.notes'),
        persistKey: PERSIST_KEYS.notesSection,
        visible: true,
        expanded: notesExpanded,
        fillAvailableSpace: true,
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
            fillAvailableSpace: true,
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
          fillAvailableSpace: true,
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
            fillAvailableSpace: true,
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
            fillAvailableSpace: true,
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
    linkedIssueForWorkspace,
    repos,
    diffs,
    gitExpanded,
    serverMetricsExpanded,
    serverAffinityExpanded,
    serverAffinityLabel,
    selectedWorkspaceSummary?.isRunning,
    terminalExpanded,
    notesExpanded,
    isTerminalVisible,
    isTerminalExpanded,
    hasUpperContent,
    upperExpanded,
    expandTerminal,
    t,
  ]);

  return (
    <div className="h-full min-h-0 border-l bg-secondary overflow-x-hidden overflow-y-auto">
      <div className="flex h-full min-h-0 flex-col divide-y border-b">
        {showDeployStatus && (
          <div
            className="flex flex-none shrink-0 items-center justify-between gap-base px-base py-half"
            data-testid="deploy-status-row"
          >
            <span className="text-sm text-low">Deploy Status</span>
            <DeployStatus
              version={appVersion}
              deploymentTimestamp={deploymentTimestamp}
              alwaysShowAge
              className="max-w-none"
            />
          </div>
        )}
        {sections
          .filter((section) => section.visible)
          .map((section) => (
            <CollapsibleSectionHeader
              key={section.persistKey}
              title={section.title}
              persistKey={section.persistKey}
              defaultExpanded={section.expanded}
              collapsible={section.collapsible ?? true}
              actions={section.actions}
              headerExtra={section.headerExtra}
              fillAvailableSpace={section.fillAvailableSpace}
              intrinsicHeight={!section.fillAvailableSpace}
            >
              <div className="flex min-h-0 flex-1 border-t w-full overflow-auto">
                {section.content}
              </div>
            </CollapsibleSectionHeader>
          ))}
      </div>
    </div>
  );
});
