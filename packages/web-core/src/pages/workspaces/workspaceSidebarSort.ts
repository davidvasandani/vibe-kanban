import type { Workspace } from '@/shared/hooks/useWorkspaces';
import type {
  WorkspaceSortBy,
  WorkspaceSortOrder,
} from '@/shared/stores/useUiPreferencesStore';

/**
 * Precomputed sort key for one workspace row.
 *
 * The point of extracting these up front is that the previous comparator parsed
 * a date string (`new Date(value).getTime()`) and lower-cased a name on *every
 * comparison*, i.e. ~2·n·log n times per sort pass. With 133 rows that is
 * thousands of `Date` allocations for a list of 133 timestamps. Decorate, sort on
 * primitives, undecorate.
 */
export interface WorkspaceSortKey {
  pinned: boolean;
  /** `null` when the workspace has no timestamp for the selected sort field. */
  ts: number | null;
  nameKey: string;
}

export function toSortKey(
  workspace: Workspace,
  sortBy: WorkspaceSortBy
): WorkspaceSortKey {
  const raw =
    sortBy === 'updated_at'
      ? workspace.latestProcessCompletedAt
      : workspace.createdAt;

  let ts: number | null = null;
  if (raw) {
    const parsed = Date.parse(raw);
    ts = Number.isNaN(parsed) ? null : parsed;
  }

  return {
    pinned: workspace.isPinned === true,
    ts,
    nameKey: workspace.name,
  };
}

/**
 * Ordering contract, unchanged from the original inline comparator:
 * pinned first, then rows missing the selected timestamp, then by timestamp in
 * the requested direction, with the name as the tiebreak both when both
 * timestamps are absent and when they are equal.
 */
export function compareSortKeys(
  a: WorkspaceSortKey,
  b: WorkspaceSortKey,
  sortOrder: WorkspaceSortOrder
): number {
  if (a.pinned !== b.pinned) {
    return a.pinned ? -1 : 1;
  }

  if (a.ts === null && b.ts === null) {
    return a.nameKey.localeCompare(b.nameKey);
  }
  if (a.ts === null) {
    return -1;
  }
  if (b.ts === null) {
    return 1;
  }

  if (a.ts === b.ts) {
    return a.nameKey.localeCompare(b.nameKey);
  }

  return sortOrder === 'asc' ? a.ts - b.ts : b.ts - a.ts;
}

export function sortWorkspaces(
  workspaces: Workspace[],
  sortBy: WorkspaceSortBy,
  sortOrder: WorkspaceSortOrder
): Workspace[] {
  return workspaces
    .map((workspace) => ({ workspace, key: toSortKey(workspace, sortBy) }))
    .sort((a, b) => compareSortKeys(a.key, b.key, sortOrder))
    .map(({ workspace }) => workspace);
}
