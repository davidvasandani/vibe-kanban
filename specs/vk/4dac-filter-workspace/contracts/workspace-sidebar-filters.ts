// Contract for packages/web-core/src/pages/workspaces/workspaceSidebarFilters.ts
// (pure; no React, no I/O).

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
  /** Normalized names (+ optional NO_ISSUE_STATUS_FILTER). */
  hiddenIssueStatusNames: string[];
  /** Lower-cased search text; '' disables search. */
  searchLower: string;
  remoteByLocalId: ReadonlyMap<string, RemoteWorkspaceLink>;
  /** issueId → normalized status name; missing = unknown (fail open). */
  issueStatusNameById: ReadonlyMap<string, string>;
}

export declare function normalizeStatusName(name: string): string;

/** AND of project, PR, issue-status and search. Returns a new array; input untouched. */
export declare function filterSidebarWorkspaces<T extends SidebarWorkspace>(
  workspaces: T[],
  criteria: SidebarWorkspaceFilterCriteria
): T[];

export interface IssueStatusFilterOption {
  value: string; // normalized name
  label: string; // first-seen display name
}

/** Dedupe by normalized name, order by min sort_order then label, append stale hidden names. */
export declare function buildIssueStatusFilterOptions(
  statuses: Pick<ProjectStatus, 'name' | 'sort_order'>[],
  hiddenIssueStatusNames: string[]
): IssueStatusFilterOption[];
