import { describe, expect, it } from 'vitest';
import type {
  Merge,
  RepoBranchStatus,
  RepoWithTargetBranch,
} from 'shared/types';
import { deriveRepoInfos } from './gitPanelRepoInfo';

const repo = (id: string, name = id): RepoWithTargetBranch =>
  ({
    id,
    name,
    display_name: name,
    target_branch: 'origin/main',
  }) as RepoWithTargetBranch;

const pullRequest = (
  repoId: string,
  number: number,
  status: 'open' | 'merged'
): Merge => ({
  type: 'pr',
  id: `pr-${number}`,
  workspace_id: 'workspace-id',
  repo_id: repoId,
  created_at: '2026-08-13T00:00:00Z',
  target_branch_name: 'main',
  pr_info: {
    number: BigInt(number),
    url: `https://github.com/example/${repoId}/pull/${number}`,
    status,
    merged_at: status === 'merged' ? '2026-08-14T00:00:00Z' : null,
    merge_commit_sha: null,
  },
});

const status = (repoId: string, merges: Merge[] = []): RepoBranchStatus =>
  ({
    repo_id: repoId,
    repo_name: repoId,
    commits_ahead: 2,
    commits_behind: 1,
    remote_commits_ahead: 3,
    merges,
    is_target_remote: true,
  }) as RepoBranchStatus;

describe('deriveRepoInfos', () => {
  it('shows a PR only on the repository whose status owns it', () => {
    const repos = ['ansible', 'homelab', 'platform-ops', 'sg-monorepo'].map(
      (name) => repo(name)
    );

    const infos = deriveRepoInfos(repos, [
      status('homelab', [pullRequest('homelab', 869, 'open')]),
      status('ansible'),
      status('platform-ops'),
      status('sg-monorepo'),
    ]);

    expect(infos.find((info) => info.id === 'homelab')).toMatchObject({
      prNumber: 869,
      prUrl: 'https://github.com/example/homelab/pull/869',
      prStatus: 'open',
    });
    expect(
      infos.filter((info) => info.id !== 'homelab').map((info) => info.prNumber)
    ).toEqual([undefined, undefined, undefined]);
  });

  it('leaves every repository PR-less while scoped status is unavailable', () => {
    expect(deriveRepoInfos([repo('one'), repo('two')], undefined)).toEqual([
      expect.objectContaining({ id: 'one', prNumber: undefined }),
      expect.objectContaining({ id: 'two', prNumber: undefined }),
    ]);
  });

  it('prefers an open PR over a merged PR within the same repository', () => {
    const [info] = deriveRepoInfos(
      [repo('homelab')],
      [
        status('homelab', [
          pullRequest('homelab', 868, 'merged'),
          pullRequest('homelab', 869, 'open'),
        ]),
      ]
    );

    expect(info).toMatchObject({ prNumber: 869, prStatus: 'open' });
  });

  it('joins branch metadata by repository ID rather than array position', () => {
    const infos = deriveRepoInfos(
      [repo('one', 'One'), repo('two', 'Two')],
      [status('two'), { ...status('one'), commits_ahead: 7 }]
    );

    expect(infos[0]).toMatchObject({
      id: 'one',
      commitsAhead: 7,
      targetBranch: 'origin/main',
    });
    expect(infos[1]).toMatchObject({
      id: 'two',
      commitsAhead: 2,
      remoteCommitsAhead: 3,
      isTargetRemote: true,
    });
  });
});
