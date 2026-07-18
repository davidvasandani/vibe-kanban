import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import type { CarouselSortMode } from '@/shared/stores/useUiPreferencesStore';

/**
 * A workspace needs feedback when its agent is blocked on a human: a pending
 * tool approval (even while a process is technically running), or unseen
 * agent output after the run stopped. Matches the sidebar "Needs Attention"
 * group and the kanban Active/Waiting split semantics.
 */
export function needsFeedback(ws: SidebarWorkspace): boolean {
  return !!ws.hasPendingApproval || (!!ws.hasUnseenActivity && !ws.isRunning);
}

/**
 * Triage tier for the default carousel sort. Lower tiers render further left:
 * 0 needs feedback, 1 stopped abnormally, 2 idle, 3 running fine on its own.
 */
export function stoppedAbnormally(ws: SidebarWorkspace): boolean {
  return (
    !ws.isRunning &&
    (ws.latestProcessStatus === 'failed' ||
      ws.latestProcessStatus === 'killed' ||
      // Interrupted runs surface a resume action in the chat; they need a
      // human just like failures do.
      ws.latestProcessStatus === 'interrupted')
  );
}

export function carouselTier(ws: SidebarWorkspace): number {
  if (needsFeedback(ws)) return 0;
  if (stoppedAbnormally(ws)) return 1;
  if (!ws.isRunning) return 2;
  return 3;
}

function toTime(value: string | undefined): number {
  if (!value) return 0;
  const time = new Date(value).getTime();
  return Number.isNaN(time) ? 0 : time;
}

function lastActivityTime(ws: SidebarWorkspace): number {
  return Math.max(toTime(ws.latestProcessCompletedAt), toTime(ws.updatedAt));
}

function comparePinnedFirst(a: SidebarWorkspace, b: SidebarWorkspace): number {
  if (!!a.isPinned !== !!b.isPinned) return a.isPinned ? -1 : 1;
  return 0;
}

function compareNeedsFeedback(
  a: SidebarWorkspace,
  b: SidebarWorkspace
): number {
  const tierDiff = carouselTier(a) - carouselTier(b);
  if (tierDiff !== 0) return tierDiff;
  // Within the needs-feedback tier, pending approvals come before
  // stopped-with-unseen-output: an approval is blocking an in-flight run.
  if (
    carouselTier(a) === 0 &&
    !!a.hasPendingApproval !== !!b.hasPendingApproval
  )
    return a.hasPendingApproval ? -1 : 1;
  return lastActivityTime(b) - lastActivityTime(a);
}

/**
 * Sort workspaces for the carousel. The `needs_feedback` mode is pure triage
 * order (pinning is deliberately ignored); the other modes follow the
 * sidebar convention of pinned-first.
 */
export function sortForCarousel(
  workspaces: SidebarWorkspace[],
  mode: CarouselSortMode
): SidebarWorkspace[] {
  const sorted = [...workspaces];
  switch (mode) {
    case 'needs_feedback':
      sorted.sort(compareNeedsFeedback);
      break;
    case 'updated_at':
      sorted.sort(
        (a, b) =>
          comparePinnedFirst(a, b) || toTime(b.updatedAt) - toTime(a.updatedAt)
      );
      break;
    case 'created_at':
      sorted.sort(
        (a, b) =>
          comparePinnedFirst(a, b) || toTime(b.createdAt) - toTime(a.createdAt)
      );
      break;
    case 'name':
      sorted.sort(
        (a, b) => comparePinnedFirst(a, b) || a.name.localeCompare(b.name)
      );
      break;
  }
  return sorted;
}
