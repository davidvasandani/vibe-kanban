import type { PullRequest, Workspace } from 'shared/remote-types';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import type { IssueWorkspaceWarningRow } from '@vibe/ui/components/IssueWorkspaceWarning';

export function getIssueWorkspaceAdvisory({
  projectId,
  issueId,
  workspaces,
  pullRequests,
  localWorkspaces,
  userId,
}: {
  projectId: string;
  issueId: string;
  workspaces: Workspace[];
  pullRequests: PullRequest[];
  localWorkspaces: SidebarWorkspace[];
  userId: string | null;
}): IssueWorkspaceWarningRow[] {
  const locals = new Map(
    localWorkspaces.map((workspace) => [workspace.id, workspace])
  );
  return workspaces
    .filter(
      (workspace) =>
        workspace.project_id === projectId &&
        workspace.issue_id === issueId &&
        !workspace.archived
    )
    .map((workspace): IssueWorkspaceWarningRow => {
      const local = workspace.local_workspace_id
        ? locals.get(workspace.local_workspace_id)
        : undefined;
      return {
        id: workspace.id,
        name: workspace.name ?? local?.name ?? workspace.id,
        branch: local?.branch ?? null,
        localWorkspaceId:
          workspace.owner_user_id === userId && local
            ? workspace.local_workspace_id
            : null,
        activity: local?.isCreating
          ? 'creating'
          : local?.isRunning === true
            ? 'running'
            : local?.isRunning === false
              ? 'idle'
              : 'unknown',
        hasOpenPr: pullRequests.some(
          (pr) =>
            pr.project_id === projectId &&
            pr.workspace_id === workspace.id &&
            pr.status === 'open'
        ),
        hasChanges: (local?.filesChanged ?? workspace.files_changed ?? 0) > 0,
      };
    })
    .sort(
      (a, b) =>
        Number(b.hasOpenPr) - Number(a.hasOpenPr) ||
        Number(b.hasChanges) - Number(a.hasChanges) ||
        a.id.localeCompare(b.id)
    );
}

export function issueWorkspaceAdvisoryKey(
  projectId: string,
  issueId: string,
  workspaces: IssueWorkspaceWarningRow[]
): string {
  return JSON.stringify([
    projectId,
    issueId,
    workspaces.map((workspace) => workspace.id).sort(),
  ]);
}
