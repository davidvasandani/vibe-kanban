import { describe, expect, it } from 'vitest';
import {
  DEFAULT_KANBAN_FILTER_STATE,
  KANBAN_ASSIGNEE_FILTER_VALUES,
  type KanbanFilterState,
} from '@/shared/stores/useUiPreferencesStore';
import type { Issue, IssueRelationship } from 'shared/remote-types';
import {
  filterKanbanIssues,
  type FilterKanbanIssuesParams,
} from './useKanbanFilters';

const IN_PROGRESS = 'status-in-progress';
const DONE = 'status-done';

function makeIssue(overrides: Partial<Issue> & Pick<Issue, 'id'>): Issue {
  return {
    project_id: 'project-1',
    issue_number: 1,
    simple_id: 'SWE-1',
    status_id: IN_PROGRESS,
    title: 'Untitled',
    description: null,
    priority: null,
    start_date: null,
    target_date: null,
    completed_at: null,
    sort_order: 0,
    parent_issue_id: null,
    parent_issue_sort_order: null,
    extension_metadata: null,
    creator_user_id: null,
    created_at: '2026-09-24T00:00:00Z',
    updated_at: '2026-09-24T00:00:00Z',
    ...overrides,
  };
}

const parent = makeIssue({
  id: 'parent',
  issue_number: 176,
  simple_id: 'SWE-176',
  title: 'Build the Meraki Network Health dashboard',
});

const subIssue = makeIssue({
  id: 'sub',
  issue_number: 190,
  simple_id: 'SWE-190',
  title: 'Add device-status and MX uplink-status collectors',
  priority: 'urgent',
  parent_issue_id: parent.id,
});

const blocker = makeIssue({
  id: 'blocker',
  issue_number: 12,
  simple_id: 'SWE-12',
  title: 'Unrelated blocker',
});

const allIssues = [parent, subIssue, blocker];

function run(
  overrides: Partial<Omit<FilterKanbanIssuesParams, 'filters'>> & {
    filters?: Partial<KanbanFilterState>;
  } = {}
): string[] {
  const { filters, ...rest } = overrides;
  const issues = rest.issues ?? allIssues;
  return filterKanbanIssues({
    issues,
    assigneesByIssue: {},
    tagsByIssue: {},
    issueRelationships: [],
    issuesById: new Map(issues.map((issue) => [issue.id, issue])),
    doneStatusIds: new Set([DONE]),
    showSubIssues: false,
    hideBlocked: false,
    currentUserId: 'user-me',
    ...rest,
    filters: { ...DEFAULT_KANBAN_FILTER_STATE, ...filters },
  }).map((issue) => issue.id);
}

describe('filterKanbanIssues sub-issue visibility', () => {
  it('hides sub-issues on the unfiltered board when the view hides them', () => {
    expect(run()).toEqual(['parent', 'blocker']);
  });

  it('treats a whitespace-only query as no query', () => {
    expect(run({ filters: { searchQuery: '   ' } })).toEqual([
      'parent',
      'blocker',
    ]);
  });

  it.each(['190', 'swe-190', 'SWE-190', 'uplink-status'])(
    'finds a hidden-by-default sub-issue when searching %s',
    (searchQuery) => {
      expect(run({ filters: { searchQuery } })).toEqual(['sub']);
    }
  );

  it('matches on the issue number alone', () => {
    const renamed = { ...subIssue, simple_id: 'OPS-1' };
    expect(
      run({ issues: [parent, renamed], filters: { searchQuery: '190' } })
    ).toEqual(['sub']);
  });

  it('returns matching parents and sub-issues together', () => {
    expect(run({ filters: { searchQuery: 'swe-1' } })).toEqual([
      'parent',
      'sub',
      'blocker',
    ]);
  });

  it('leaves results unchanged when the view already shows sub-issues', () => {
    expect(run({ showSubIssues: true })).toEqual(['parent', 'sub', 'blocker']);
    expect(
      run({ showSubIssues: true, filters: { searchQuery: '190' } })
    ).toEqual(['sub']);
  });
});

describe('filterKanbanIssues keeps deliberate filters on search results', () => {
  it('applies the priority filter', () => {
    expect(
      run({ filters: { searchQuery: '190', priorities: ['low'] } })
    ).toEqual([]);
    expect(
      run({ filters: { searchQuery: '190', priorities: ['urgent'] } })
    ).toEqual(['sub']);
  });

  it('applies the assignee "self" filter', () => {
    const filters = {
      searchQuery: '190',
      assigneeIds: [KANBAN_ASSIGNEE_FILTER_VALUES.SELF],
    };
    expect(run({ filters })).toEqual([]);
    expect(run({ filters, assigneesByIssue: { sub: ['user-me'] } })).toEqual([
      'sub',
    ]);
  });

  it('applies the tag filter', () => {
    const filters = { searchQuery: '190', tagIds: ['tag-network'] };
    expect(run({ filters })).toEqual([]);
    expect(run({ filters, tagsByIssue: { sub: ['tag-network'] } })).toEqual([
      'sub',
    ]);
  });

  it('applies hide blocked', () => {
    const blocking: IssueRelationship = {
      id: 'rel-1',
      issue_id: blocker.id,
      related_issue_id: subIssue.id,
      relationship_type: 'blocking',
      created_at: '2026-09-24T00:00:00Z',
    };
    const params = {
      hideBlocked: true,
      issueRelationships: [blocking],
      filters: { searchQuery: '190' },
    };
    expect(run(params)).toEqual([]);
    const resolvedBlocker = { ...blocker, status_id: DONE };
    expect(
      run({ ...params, issues: [parent, subIssue, resolvedBlocker] })
    ).toEqual(['sub']);
  });

  it('does not mutate the filter state it is given', () => {
    const filters: KanbanFilterState = {
      ...DEFAULT_KANBAN_FILTER_STATE,
      searchQuery: '190',
    };
    const snapshot = structuredClone(filters);
    filterKanbanIssues({
      issues: allIssues,
      assigneesByIssue: {},
      tagsByIssue: {},
      issueRelationships: [],
      issuesById: new Map(allIssues.map((issue) => [issue.id, issue])),
      doneStatusIds: new Set([DONE]),
      filters,
      showSubIssues: false,
      hideBlocked: false,
      currentUserId: null,
    });
    expect(filters).toEqual(snapshot);
  });
});
