import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import type { ProjectStatus } from 'shared/remote-types';
import type { WorkspacePrFilter } from '@/shared/stores/useUiPreferencesStore';

export const NO_PROJECT_FILTER = '__no_project__';
export const NO_ISSUE_STATUS_FILTER = '__no_issue__';

export interface RemoteWorkspaceLink {
  projectId: string;
  issueId: string | null;
}

export interface SidebarWorkspaceFilterCriteria {
  projectIds: string[];
  prFilter: WorkspacePrFilter;
  /** Normalized status names to hide, optionally with NO_ISSUE_STATUS_FILTER */
  hiddenIssueStatusNames: string[];
  /** Lower-cased search text; empty disables search */
  searchLower: string;
  /** Local workspace ID → linked remote project/issue */
  remoteByLocalId: ReadonlyMap<string, RemoteWorkspaceLink>;
  /** Issue ID → normalized status name. Missing means unknown. */
  issueStatusNameById: ReadonlyMap<string, string>;
}

/**
 * Status ids are per-project, so the global preference matches statuses by
 * name, ignoring case and surrounding whitespace.
 */
export function normalizeStatusName(name: string): string {
  return name.trim().toLowerCase();
}

/**
 * Applies the sidebar filters (project, PR, issue status, search) as an AND.
 * The issue-status filter fails open: a workspace is hidden only when its
 * linked issue's status is known and hidden.
 */
export function filterSidebarWorkspaces<T extends SidebarWorkspace>(
  workspaces: T[],
  criteria: SidebarWorkspaceFilterCriteria
): T[] {
  const {
    projectIds,
    prFilter,
    hiddenIssueStatusNames,
    searchLower,
    remoteByLocalId,
    issueStatusNameById,
  } = criteria;

  const includeNoProject = projectIds.includes(NO_PROJECT_FILTER);
  const realProjectIds = projectIds.filter((id) => id !== NO_PROJECT_FILTER);
  const hideNoIssue = hiddenIssueStatusNames.includes(NO_ISSUE_STATUS_FILTER);
  const hiddenStatusNames = new Set(
    hiddenIssueStatusNames.filter((name) => name !== NO_ISSUE_STATUS_FILTER)
  );

  return workspaces.filter((ws) => {
    const remote = remoteByLocalId.get(ws.id);

    if (projectIds.length > 0) {
      if (!remote) {
        if (!includeNoProject) return false;
      } else if (!realProjectIds.includes(remote.projectId)) {
        return false;
      }
    }

    if (prFilter === 'has_pr' && !ws.prStatus) return false;
    if (prFilter === 'no_pr' && ws.prStatus) return false;

    if (hiddenIssueStatusNames.length > 0) {
      const issueId = remote?.issueId ?? null;
      if (!issueId) {
        if (hideNoIssue) return false;
      } else {
        const statusName = issueStatusNameById.get(issueId);
        if (statusName !== undefined && hiddenStatusNames.has(statusName)) {
          return false;
        }
      }
    }

    if (
      searchLower &&
      !ws.name.toLowerCase().includes(searchLower) &&
      !ws.branch.toLowerCase().includes(searchLower)
    ) {
      return false;
    }

    return true;
  });
}

export interface IssueStatusFilterOption {
  /** Normalized status name */
  value: string;
  /** First-seen display name */
  label: string;
}

/**
 * One option per distinct status name across projects, in board order
 * (lowest sort_order first). Hidden names that no loaded project has are
 * appended so they can still be un-hidden.
 */
export function buildIssueStatusFilterOptions(
  statuses: Pick<ProjectStatus, 'name' | 'sort_order'>[],
  hiddenIssueStatusNames: string[]
): IssueStatusFilterOption[] {
  const byName = new Map<string, { label: string; sortOrder: number }>();
  for (const status of statuses) {
    const value = normalizeStatusName(status.name);
    if (!value) continue;
    const existing = byName.get(value);
    if (!existing) {
      byName.set(value, {
        label: status.name.trim(),
        sortOrder: status.sort_order,
      });
    } else if (status.sort_order < existing.sortOrder) {
      existing.sortOrder = status.sort_order;
    }
  }

  const options = Array.from(byName.entries())
    .sort(
      ([, a], [, b]) =>
        a.sortOrder - b.sortOrder || a.label.localeCompare(b.label)
    )
    .map(([value, { label }]) => ({ value, label }));

  const seen = new Set(byName.keys());
  for (const name of hiddenIssueStatusNames) {
    if (name === NO_ISSUE_STATUS_FILTER || seen.has(name)) continue;
    seen.add(name);
    options.push({ value: name, label: name });
  }

  return options;
}
