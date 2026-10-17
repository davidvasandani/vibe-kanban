import { describe, expect, it } from 'vitest';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import {
  NO_ISSUE_STATUS_FILTER,
  NO_PROJECT_FILTER,
  buildIssueStatusFilterOptions,
  filterSidebarWorkspaces,
  normalizeStatusName,
  type RemoteWorkspaceLink,
  type SidebarWorkspaceFilterCriteria,
} from './workspaceSidebarFilters';

function workspace(
  id: string,
  overrides: Partial<SidebarWorkspace> = {}
): SidebarWorkspace {
  return {
    id,
    name: id,
    branch: `vk/${id}`,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z',
    description: '',
    ...overrides,
  };
}

function ids(workspaces: SidebarWorkspace[]): string[] {
  return workspaces.map(({ id }) => id);
}

function criteria(
  overrides: Partial<SidebarWorkspaceFilterCriteria> = {}
): SidebarWorkspaceFilterCriteria {
  return {
    projectIds: [],
    prFilter: 'all',
    hiddenIssueStatusNames: [],
    searchLower: '',
    remoteByLocalId: new Map(),
    issueStatusNameById: new Map(),
    ...overrides,
  };
}

// Two projects, each with its own "In review" status (different ids)
const remoteByLocalId = new Map<string, RemoteWorkspaceLink>([
  ['review-a', { projectId: 'proj-a', issueId: 'issue-a1' }],
  ['review-b', { projectId: 'proj-b', issueId: 'issue-b1' }],
  ['progress', { projectId: 'proj-a', issueId: 'issue-a2' }],
  ['unlinked', { projectId: 'proj-a', issueId: null }],
  ['not-loaded', { projectId: 'proj-b', issueId: 'issue-b9' }],
]);
const issueStatusNameById = new Map<string, string>([
  ['issue-a1', 'in review'],
  ['issue-b1', 'in review'],
  ['issue-a2', 'in progress'],
]);
const all = [
  workspace('review-a'),
  workspace('review-b'),
  workspace('progress'),
  workspace('unlinked'),
  workspace('not-loaded'),
  workspace('local-only'),
];

describe('normalizeStatusName', () => {
  it('trims and lower-cases', () => {
    expect(normalizeStatusName('  In Review ')).toBe('in review');
  });
});

describe('filterSidebarWorkspaces — issue status', () => {
  it('shows everything when no status is hidden', () => {
    expect(
      ids(
        filterSidebarWorkspaces(
          all,
          criteria({ remoteByLocalId, issueStatusNameById })
        )
      )
    ).toEqual(ids(all));
  });

  it('hides a status by name across projects', () => {
    const result = filterSidebarWorkspaces(
      all,
      criteria({
        hiddenIssueStatusNames: ['in review'],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(result)).toEqual([
      'progress',
      'unlinked',
      'not-loaded',
      'local-only',
    ]);
  });

  it('hides only the chosen status', () => {
    const result = filterSidebarWorkspaces(
      all,
      criteria({
        hiddenIssueStatusNames: ['in progress'],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(result)).toContain('review-a');
    expect(ids(result)).toContain('review-b');
    expect(ids(result)).not.toContain('progress');
  });

  it('fails open while issue statuses are not loaded', () => {
    const result = filterSidebarWorkspaces(
      all,
      criteria({
        hiddenIssueStatusNames: ['in review'],
        remoteByLocalId,
        issueStatusNameById: new Map(),
      })
    );
    expect(ids(result)).toEqual(ids(all));
  });

  it('keeps workspaces without a linked issue unless No issue is hidden', () => {
    const shown = filterSidebarWorkspaces(
      all,
      criteria({
        hiddenIssueStatusNames: ['in review'],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(shown)).toContain('unlinked');
    expect(ids(shown)).toContain('local-only');

    const hidden = filterSidebarWorkspaces(
      all,
      criteria({
        hiddenIssueStatusNames: [NO_ISSUE_STATUS_FILTER],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(hidden)).toEqual([
      'review-a',
      'review-b',
      'progress',
      'not-loaded',
    ]);
  });

  it('applies to workspaces that need attention', () => {
    const needsAttention = workspace('review-a', { hasPendingApproval: true });
    const result = filterSidebarWorkspaces(
      [needsAttention, workspace('progress', { hasPendingApproval: true })],
      criteria({
        hiddenIssueStatusNames: ['in review'],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(result)).toEqual(['progress']);
  });

  it('composes with project, PR and search filters', () => {
    const withPr = [
      workspace('review-a', { prStatus: 'open' }),
      workspace('progress', { prStatus: 'open', name: 'Fix login' }),
      workspace('unlinked', { prStatus: 'open', name: 'Fix logout' }),
      workspace('review-b', { prStatus: 'open' }),
      workspace('not-loaded'),
    ];
    const result = filterSidebarWorkspaces(
      withPr,
      criteria({
        projectIds: ['proj-a'],
        prFilter: 'has_pr',
        hiddenIssueStatusNames: ['in review'],
        searchLower: 'fix',
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(result)).toEqual(['progress', 'unlinked']);
  });

  it('keeps the existing No project behaviour', () => {
    const result = filterSidebarWorkspaces(
      all,
      criteria({ projectIds: [NO_PROJECT_FILTER], remoteByLocalId })
    );
    expect(ids(result)).toEqual(['local-only']);
  });

  it('does not mutate its input', () => {
    const input = [...all];
    filterSidebarWorkspaces(
      input,
      criteria({
        hiddenIssueStatusNames: ['in review'],
        remoteByLocalId,
        issueStatusNameById,
      })
    );
    expect(ids(input)).toEqual(ids(all));
  });
});

describe('buildIssueStatusFilterOptions', () => {
  it('dedupes names case-insensitively in board order', () => {
    const options = buildIssueStatusFilterOptions(
      [
        { name: 'Done', sort_order: 4 },
        { name: 'In review', sort_order: 3 },
        { name: 'To do', sort_order: 0 },
        { name: 'In Review ', sort_order: 2 },
        { name: 'In progress', sort_order: 1 },
      ],
      []
    );
    expect(options).toEqual([
      { value: 'to do', label: 'To do' },
      { value: 'in progress', label: 'In progress' },
      { value: 'in review', label: 'In review' },
      { value: 'done', label: 'Done' },
    ]);
  });

  it('keeps hidden names that no loaded project has', () => {
    const options = buildIssueStatusFilterOptions(
      [{ name: 'Done', sort_order: 0 }],
      ['done', 'blocked', NO_ISSUE_STATUS_FILTER]
    );
    expect(options).toEqual([
      { value: 'done', label: 'Done' },
      { value: 'blocked', label: 'blocked' },
    ]);
  });
});
