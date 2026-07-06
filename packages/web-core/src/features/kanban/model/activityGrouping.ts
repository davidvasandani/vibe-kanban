/**
 * Helpers for splitting the "In progress" kanban column into an "Active"
 * group (an agent is running on a linked workspace) and a "Waiting for
 * feedback" group (agent finished, paused on tool approval, or no live
 * workspace signal). Grouping is derived, ephemeral UI state — it never
 * touches an issue's status_id or sort_order.
 */

export type ActivityGroup = 'active' | 'waiting';

export interface ActivityGroups {
  active: string[];
  waiting: string[];
  /** Headers only render when the column actually contains both groups. */
  showHeaders: boolean;
}

/**
 * Matches the seeded "In progress" status. The backend also identifies this
 * status by name (see sync_issue_from_workspace_created in
 * crates/remote/src/db/issues.rs); case/whitespace tolerance here only
 * forgives cosmetic renames.
 */
export function isInProgressStatus(name: string): boolean {
  return name.trim().toLowerCase() === 'in progress';
}

/**
 * A workspace counts as actively working only while its agent runs
 * unattended. A run paused on a pending tool approval is waiting on the
 * user, mirroring IssueWorkspaceCard's hand-icon semantics.
 */
export function isWorkspaceActive(workspace: {
  isRunning?: boolean;
  hasPendingApproval?: boolean;
}): boolean {
  return workspace.isRunning === true && workspace.hasPendingApproval !== true;
}

/**
 * Stable partition: active issues first, then waiting, preserving the
 * incoming (already sorted) order within each group.
 */
export function partitionByActivity(
  issueIds: string[],
  activeIssueIds: ReadonlySet<string>
): string[] {
  const active: string[] = [];
  const waiting: string[] = [];
  for (const id of issueIds) {
    (activeIssueIds.has(id) ? active : waiting).push(id);
  }
  return [...active, ...waiting];
}

export function buildActivityGroups(
  issueIds: string[],
  activeIssueIds: ReadonlySet<string>
): ActivityGroups {
  const active: string[] = [];
  const waiting: string[] = [];
  for (const id of issueIds) {
    (activeIssueIds.has(id) ? active : waiting).push(id);
  }
  return {
    active,
    waiting,
    showHeaders: active.length > 0 && waiting.length > 0,
  };
}
