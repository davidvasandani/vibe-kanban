import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import type {
  WorkspaceSortBy,
  WorkspaceSortOrder,
} from '@/shared/stores/useUiPreferencesStore';

function toTimestamp(value: string | undefined): number | null {
  if (!value) return null;

  const timestamp = new Date(value).getTime();
  return Number.isNaN(timestamp) ? null : timestamp;
}

function getWorkspaceSortTimestamp(
  workspace: SidebarWorkspace,
  sortBy: WorkspaceSortBy
): number | null {
  if (sortBy === 'created_at') {
    return toTimestamp(workspace.createdAt);
  }

  return (
    toTimestamp(workspace.latestProcessCompletedAt) ??
    toTimestamp(workspace.updatedAt)
  );
}

function compareWorkspaceIdentity(
  a: SidebarWorkspace,
  b: SidebarWorkspace
): number {
  return a.name.localeCompare(b.name) || a.id.localeCompare(b.id);
}

export function compareWorkspaces(
  a: SidebarWorkspace,
  b: SidebarWorkspace,
  sortBy: WorkspaceSortBy,
  sortOrder: WorkspaceSortOrder
): number {
  if (!!a.isPinned !== !!b.isPinned) {
    return a.isPinned ? -1 : 1;
  }

  const aTimestamp = getWorkspaceSortTimestamp(a, sortBy);
  const bTimestamp = getWorkspaceSortTimestamp(b, sortBy);

  if (aTimestamp === null && bTimestamp === null) {
    return compareWorkspaceIdentity(a, b);
  }
  if (aTimestamp === null) return 1;
  if (bTimestamp === null) return -1;

  if (aTimestamp === bTimestamp) {
    return compareWorkspaceIdentity(a, b);
  }

  return sortOrder === 'asc'
    ? aTimestamp - bTimestamp
    : bTimestamp - aTimestamp;
}

export function sortWorkspaces(
  workspaces: SidebarWorkspace[],
  sortBy: WorkspaceSortBy,
  sortOrder: WorkspaceSortOrder
): SidebarWorkspace[] {
  return [...workspaces].sort((a, b) =>
    compareWorkspaces(a, b, sortBy, sortOrder)
  );
}
