import { useMemo } from 'react';
import {
  KANBAN_ASSIGNEE_FILTER_VALUES,
  type KanbanFilterState,
} from '@/shared/stores/useUiPreferencesStore';
import type {
  Issue,
  IssueAssignee,
  IssueRelationship,
  IssueTag,
  IssuePriority,
} from 'shared/remote-types';

type UseKanbanFiltersParams = {
  issues: Issue[];
  issueAssignees: IssueAssignee[];
  issueTags: IssueTag[];
  issueRelationships: IssueRelationship[];
  issuesById: Map<string, Issue>;
  doneStatusIds: Set<string>;
  filters: KanbanFilterState;
  showSubIssues: boolean;
  hideBlocked: boolean;
  currentUserId: string | null;
};

type UseKanbanFiltersResult = {
  filteredIssues: Issue[];
};

export type FilterKanbanIssuesParams = {
  issues: Issue[];
  assigneesByIssue: Record<string, string[]>;
  tagsByIssue: Record<string, string[]>;
  issueRelationships: IssueRelationship[];
  issuesById: Map<string, Issue>;
  doneStatusIds: Set<string>;
  filters: KanbanFilterState;
  showSubIssues: boolean;
  hideBlocked: boolean;
  currentUserId: string | null;
};

export const PRIORITY_ORDER: Record<IssuePriority, number> = {
  urgent: 0,
  high: 1,
  medium: 2,
  low: 3,
};

export function filterKanbanIssues({
  issues,
  assigneesByIssue,
  tagsByIssue,
  issueRelationships,
  issuesById,
  doneStatusIds,
  filters,
  showSubIssues,
  hideBlocked,
  currentUserId,
}: FilterKanbanIssuesParams): Issue[] {
  let result = issues;

  const query = filters.searchQuery.trim().toLowerCase();

  // Filter sub-issues based on per-project preference. The preference only
  // curates the unfiltered board: an explicit search must still find a
  // matching sub-issue, otherwise it is unreachable from the Team view.
  if (!showSubIssues && !query) {
    result = result.filter((issue) => issue.parent_issue_id === null);
  }

  // Text search (title + short ID)
  if (query) {
    result = result.filter((issue) => {
      if (issue.title.toLowerCase().includes(query)) {
        return true;
      }

      const simpleId = issue.simple_id.toLowerCase();
      if (simpleId.includes(query)) {
        return true;
      }

      const issueNumber = String(issue.issue_number);
      return issueNumber.includes(query);
    });
  }

  // Priority filter (OR within)
  if (filters.priorities.length > 0) {
    result = result.filter(
      (issue) =>
        issue.priority !== null && filters.priorities.includes(issue.priority)
    );
  }

  // Assignee filter (OR within)
  if (filters.assigneeIds.length > 0) {
    const includeUnassigned = filters.assigneeIds.includes(
      KANBAN_ASSIGNEE_FILTER_VALUES.UNASSIGNED
    );
    const selectedAssigneeIds = new Set(
      filters.assigneeIds.flatMap((assigneeId) => {
        if (assigneeId === KANBAN_ASSIGNEE_FILTER_VALUES.SELF) {
          return currentUserId ? [currentUserId] : [];
        }
        if (assigneeId === KANBAN_ASSIGNEE_FILTER_VALUES.UNASSIGNED) {
          return [];
        }
        return [assigneeId];
      })
    );

    result = result.filter((issue) => {
      const issueAssigneeIds = assigneesByIssue[issue.id] ?? [];

      // Check for 'unassigned' special case
      if (includeUnassigned) {
        if (issueAssigneeIds.length === 0) return true;
      }

      // Check if any of the issue's assignees match the filter
      return issueAssigneeIds.some((assigneeId) =>
        selectedAssigneeIds.has(assigneeId)
      );
    });
  }

  // Tags filter (OR within)
  if (filters.tagIds.length > 0) {
    result = result.filter((issue) => {
      const issueTagIds = tagsByIssue[issue.id] ?? [];
      return issueTagIds.some((tagId) => filters.tagIds.includes(tagId));
    });
  }

  // Hide blocked: filter out issues that are blocked by an unresolved issue
  if (hideBlocked) {
    result = result.filter((issue) => {
      return !issueRelationships.some((r) => {
        if (r.relationship_type !== 'blocking') return false;
        if (r.related_issue_id !== issue.id) return false;
        const blockingIssue = issuesById.get(r.issue_id);
        if (blockingIssue == null) return false;
        // Blocker is resolved if it's in a done status
        return !doneStatusIds.has(blockingIssue.status_id);
      });
    });
  }

  // Note: Sorting is handled in KanbanContainer after grouping by status
  // so that sort order is applied within each column

  return result;
}

export function useKanbanFilters({
  issues,
  issueAssignees,
  issueTags,
  issueRelationships,
  issuesById,
  doneStatusIds,
  filters,
  showSubIssues,
  hideBlocked,
  currentUserId,
}: UseKanbanFiltersParams): UseKanbanFiltersResult {
  // Create lookup maps for efficient filtering
  const assigneesByIssue = useMemo(() => {
    const map: Record<string, string[]> = {};
    for (const ia of issueAssignees) {
      if (!map[ia.issue_id]) {
        map[ia.issue_id] = [];
      }
      map[ia.issue_id].push(ia.user_id);
    }
    return map;
  }, [issueAssignees]);

  const tagsByIssue = useMemo(() => {
    const map: Record<string, string[]> = {};
    for (const it of issueTags) {
      if (!map[it.issue_id]) {
        map[it.issue_id] = [];
      }
      map[it.issue_id].push(it.tag_id);
    }
    return map;
  }, [issueTags]);

  const filteredIssues = useMemo(
    () =>
      filterKanbanIssues({
        issues,
        assigneesByIssue,
        tagsByIssue,
        issueRelationships,
        issuesById,
        doneStatusIds,
        filters,
        showSubIssues,
        hideBlocked,
        currentUserId,
      }),
    [
      issues,
      filters,
      assigneesByIssue,
      tagsByIssue,
      showSubIssues,
      hideBlocked,
      issueRelationships,
      issuesById,
      doneStatusIds,
      currentUserId,
    ]
  );

  return {
    filteredIssues,
  };
}
