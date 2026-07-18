import { Component, useCallback, useRef } from 'react';
import type { FocusEvent, ReactNode } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  ArrowSquareOutIcon,
  CircleIcon,
  HandIcon,
  SpinnerIcon,
  TriangleIcon,
} from '@phosphor-icons/react';
import { cn } from '@/shared/lib/utils';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import { useWorkspaceRecord } from '@/shared/hooks/useWorkspaceRecord';
import { useWorkspaceSessions } from '@/shared/hooks/useWorkspaceSessions';
import { useWorkspaceRepo } from '@/shared/hooks/useWorkspaceRepo';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { workspacesApi } from '@/shared/lib/api';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { ExecutionProcessesProvider } from '@/shared/providers/ExecutionProcessesProvider';
import { WorkspacesMainContainer } from '@/pages/workspaces/WorkspacesMainContainer';
import { needsFeedback, stoppedAbnormally } from './carouselSort';

type ColumnStatus =
  | 'needsFeedback'
  | 'failed'
  | 'interrupted'
  | 'running'
  | 'idle';

function columnStatus(workspace: SidebarWorkspace): ColumnStatus {
  if (needsFeedback(workspace)) return 'needsFeedback';
  if (stoppedAbnormally(workspace)) {
    return workspace.latestProcessStatus === 'interrupted'
      ? 'interrupted'
      : 'failed';
  }
  if (workspace.isRunning) return 'running';
  return 'idle';
}

function StatusBadge({ workspace }: { workspace: SidebarWorkspace }) {
  const { t } = useTranslation('common');
  const status = columnStatus(workspace);

  const icon =
    status === 'needsFeedback' ? (
      workspace.hasPendingApproval ? (
        <HandIcon className="size-icon-xs text-brand" weight="fill" />
      ) : (
        <CircleIcon className="size-icon-xs text-brand" weight="fill" />
      )
    ) : status === 'failed' || status === 'interrupted' ? (
      <TriangleIcon className="size-icon-xs text-error" weight="fill" />
    ) : status === 'running' ? (
      <SpinnerIcon className="size-icon-xs animate-spin text-success" />
    ) : null;

  return (
    <span
      className={cn(
        'inline-flex shrink-0 items-center gap-half rounded-sm border border-border bg-secondary px-half py-px text-xs',
        status === 'needsFeedback' ? 'text-normal' : 'text-low'
      )}
    >
      {icon}
      {t(`workspaces.carousel.status.${status}`)}
    </span>
  );
}

/**
 * One broken workspace must not blank the whole strip: contain render errors
 * to the column that threw.
 */
class ColumnErrorBoundary extends Component<
  { fallback: ReactNode; children: ReactNode },
  { hasError: boolean }
> {
  state = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: unknown) {
    console.error('Carousel column crashed:', error);
  }

  render() {
    return this.state.hasError ? this.props.fallback : this.props.children;
  }
}

/**
 * The live chat for one carousel column. Mounted only while the column is
 * inside the carousel's render window; hooks are scoped by explicit
 * workspaceId so many instances can coexist (no route-bound providers).
 */
function LiveColumnBody({
  workspace: summaryWorkspace,
}: {
  workspace: SidebarWorkspace;
}) {
  const workspaceId = summaryWorkspace.id;
  const { data: workspace, isLoading: isWorkspaceLoading } =
    useWorkspaceRecord(workspaceId);
  const {
    sessions,
    selectedSession,
    selectedSessionId,
    selectSession,
    isNewSessionMode,
    startNewSession,
    isLoading: isSessionsLoading,
  } = useWorkspaceSessions(workspaceId);
  const { repos } = useWorkspaceRepo(workspaceId);

  return (
    <ExecutionProcessesProvider sessionId={selectedSessionId}>
      <WorkspacesMainContainer
        selectedWorkspace={workspace ?? null}
        selectedSession={selectedSession}
        selectedSessionId={selectedSessionId}
        sessions={sessions}
        repos={repos}
        onSelectSession={selectSession}
        isLoading={isWorkspaceLoading || isSessionsLoading}
        isNewSessionMode={isNewSessionMode}
        onStartNewSession={startNewSession}
        diffStatsOverride={{
          files_changed: summaryWorkspace.filesChanged ?? 0,
          lines_added: summaryWorkspace.linesAdded ?? 0,
          lines_removed: summaryWorkspace.linesRemoved ?? 0,
        }}
        hideContextBar
      />
    </ExecutionProcessesProvider>
  );
}

export interface WorkspaceCarouselColumnProps {
  workspace: SidebarWorkspace;
  /** Whether the full chat is mounted (mount windowing). */
  live: boolean;
  /** Reports whether focus is inside this column (used to freeze ordering). */
  onChatFocusChange?: (workspaceId: string, focused: boolean) => void;
}

export function WorkspaceCarouselColumn({
  workspace,
  live,
  onChatFocusChange,
}: WorkspaceCarouselColumnProps) {
  const { t } = useTranslation('common');
  const appNavigation = useAppNavigation();
  const queryClient = useQueryClient();
  const markSeenInFlightRef = useRef(false);

  // Unlike the full view, a column never marks the workspace seen on mount.
  // Chat editors autofocus when they mount, so focus alone is NOT a user
  // signal — only a real interaction (pointer or keyboard inside the column)
  // counts as "I've looked at this agent". This both preserves the
  // unseen-activity signal the default sort is built on and drives the
  // order freeze.
  const handleInteraction = useCallback(() => {
    onChatFocusChange?.(workspace.id, true);
    if (workspace.hasUnseenActivity && !markSeenInFlightRef.current) {
      markSeenInFlightRef.current = true;
      workspacesApi
        .markSeen(workspace.id)
        .then(() => {
          queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all });
        })
        .catch((error) => {
          console.warn('Failed to mark workspace as seen:', error);
        })
        .finally(() => {
          markSeenInFlightRef.current = false;
        });
    }
  }, [
    workspace.id,
    workspace.hasUnseenActivity,
    onChatFocusChange,
    queryClient,
  ]);

  const handleBlurCapture = useCallback(
    (event: FocusEvent<HTMLDivElement>) => {
      // Ignore focus moves within the column.
      if (
        event.relatedTarget &&
        event.currentTarget.contains(event.relatedTarget as Node)
      ) {
        return;
      }
      onChatFocusChange?.(workspace.id, false);
    },
    [workspace.id, onChatFocusChange]
  );

  return (
    <section
      className="flex h-full w-[420px] shrink-0 flex-col border-r border-border bg-primary"
      data-carousel-column={workspace.id}
      onPointerDownCapture={handleInteraction}
      onKeyDownCapture={handleInteraction}
      onBlurCapture={handleBlurCapture}
      aria-label={workspace.name}
    >
      {/* Header stays outside the column's vertical scroller so it travels
          with the column during horizontal scrolls. */}
      <header className="flex shrink-0 items-center gap-base border-b border-border bg-secondary px-base py-half">
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm text-normal">{workspace.name}</div>
          <div className="truncate text-xs text-low">{workspace.branch}</div>
        </div>
        <StatusBadge workspace={workspace} />
        <button
          type="button"
          onClick={() => appNavigation.goToWorkspace(workspace.id)}
          className="shrink-0 text-low transition-colors hover:text-normal"
          aria-label={t('workspaces.carousel.openFullView')}
          title={t('workspaces.carousel.openFullView')}
        >
          <ArrowSquareOutIcon className="size-icon-base" />
        </button>
      </header>
      {/* The body owns no horizontal scrolling (overflow-x-hidden) so the
          outer strip keeps exclusive ownership of horizontal pan gestures. */}
      <div className="min-h-0 flex-1 overflow-x-hidden">
        {live ? (
          <ColumnErrorBoundary
            fallback={
              <div className="flex h-full items-center justify-center px-base text-center">
                <span className="text-sm text-error">
                  {t('workspaces.carousel.columnError')}
                </span>
              </div>
            }
          >
            <LiveColumnBody workspace={workspace} />
          </ColumnErrorBoundary>
        ) : (
          <div className="flex h-full items-center justify-center px-base text-center">
            <span className="text-sm text-low opacity-60">
              {t('workspaces.carousel.columnPlaceholder')}
            </span>
          </div>
        )}
      </div>
    </section>
  );
}
