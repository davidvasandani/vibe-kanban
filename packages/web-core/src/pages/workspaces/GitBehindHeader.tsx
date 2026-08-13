import { useMemo } from 'react';
import { useBranchStatus } from '@/shared/hooks/useBranchStatus';
import type { RepoBranchStatus, RepoWithTargetBranch } from 'shared/types';

type RepoIdentity = Pick<RepoWithTargetBranch, 'id' | 'name' | 'display_name'>;
type BehindStatus = Pick<RepoBranchStatus, 'repo_id' | 'commits_behind'>;

export interface GitBehindHeaderStatus {
  visibleText: string;
  accessibleText: string;
}

export function deriveGitBehindHeaderStatus(
  repos: RepoIdentity[],
  branchStatus: BehindStatus[] | undefined
): GitBehindHeaderStatus | null {
  if (!branchStatus) return null;

  const statusByRepoId = new Map(
    branchStatus.map((status) => [status.repo_id, status])
  );
  const entries = repos.flatMap((repo) => {
    const commitsBehind = statusByRepoId.get(repo.id)?.commits_behind;
    if (commitsBehind == null || commitsBehind <= 0) return [];
    return [
      {
        name: repo.display_name || repo.name,
        commitsBehind,
      },
    ];
  });

  if (entries.length === 0) return null;

  const accessibleText = entries
    .map(
      ({ name, commitsBehind }) =>
        `${name} is ${commitsBehind} ${
          commitsBehind === 1 ? 'commit' : 'commits'
        } behind`
    )
    .join('; ');

  return {
    visibleText:
      repos.length === 1
        ? `${entries[0].commitsBehind} behind`
        : entries
            .map(({ name, commitsBehind }) => `${name} ${commitsBehind}`)
            .join(' · '),
    accessibleText,
  };
}

interface GitBehindHeaderProps {
  workspaceId?: string;
  repos: RepoWithTargetBranch[];
}

export function GitBehindHeader({ workspaceId, repos }: GitBehindHeaderProps) {
  const { data: branchStatus } = useBranchStatus(workspaceId);
  const status = useMemo(
    () => deriveGitBehindHeaderStatus(repos, branchStatus),
    [repos, branchStatus]
  );

  if (!status) return null;

  return (
    <span
      className="min-w-0 max-w-40 truncate text-sm text-low"
      title={status.accessibleText}
      aria-label={status.accessibleText}
    >
      {status.visibleText}
    </span>
  );
}
