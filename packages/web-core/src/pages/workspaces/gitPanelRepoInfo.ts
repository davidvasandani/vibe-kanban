import type { RepoInfo } from '@vibe/ui/components/GitPanel';
import type {
  Merge,
  RepoBranchStatus,
  RepoWithTargetBranch,
} from 'shared/types';

export function deriveRepoInfos(
  repos: RepoWithTargetBranch[],
  branchStatus: RepoBranchStatus[] | undefined
): RepoInfo[] {
  const statusByRepoId = new Map(
    branchStatus?.map((status) => [status.repo_id, status]) ?? []
  );

  return repos.map((repo) => {
    const repoStatus = statusByRepoId.get(repo.id);
    const openPR = repoStatus?.merges.find(
      (merge): merge is Extract<Merge, { type: 'pr' }> =>
        merge.type === 'pr' && merge.pr_info.status === 'open'
    );
    const mergedPR = repoStatus?.merges.find(
      (merge): merge is Extract<Merge, { type: 'pr' }> =>
        merge.type === 'pr' && merge.pr_info.status === 'merged'
    );
    const relevantPR = openPR ?? mergedPR;

    return {
      id: repo.id,
      name: repo.display_name || repo.name,
      targetBranch: repo.target_branch || 'main',
      commitsAhead: repoStatus?.commits_ahead ?? 0,
      commitsBehind: repoStatus?.commits_behind ?? 0,
      remoteCommitsAhead: repoStatus?.remote_commits_ahead ?? 0,
      prNumber: relevantPR ? Number(relevantPR.pr_info.number) : undefined,
      prUrl: relevantPR?.pr_info.url,
      prStatus: relevantPR?.pr_info.status,
      isTargetRemote: repoStatus?.is_target_remote ?? false,
    };
  });
}
