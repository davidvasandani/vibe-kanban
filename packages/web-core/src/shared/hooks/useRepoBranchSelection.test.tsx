/* @vitest-environment jsdom */
import React, { act, useEffect } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { GitBranch, Repo } from 'shared/types';
import { repoApi } from '@/shared/lib/api';
import {
  useRepoBranchSelection,
  type RepoBranchConfig,
} from './useRepoBranchSelection';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@/shared/lib/api', () => ({
  repoApi: { getBranches: vi.fn() },
}));

type Selection = {
  configs: RepoBranchConfig[];
  isLoading: boolean;
  setRepoBranch: (repoId: string, branch: string) => void;
  getWorkspaceRepoInputs: () => Array<{
    repo_id: string;
    target_branch: string;
  }>;
};

const repo = (defaultTargetBranch: string | null = null) =>
  ({
    id: 'repo-1',
    display_name: 'Repository',
    default_target_branch: defaultTargetBranch,
  }) as Repo;

const branch = (name: string, { current = false, remote = false } = {}) =>
  ({
    name,
    is_current: current,
    is_remote: remote,
    last_commit_date: new Date(0),
  }) as GitBranch;

function Probe({
  repos,
  initialBranch,
  onChange,
}: {
  repos: Repo[];
  initialBranch?: string;
  onChange: (selection: Selection) => void;
}) {
  const selection = useRepoBranchSelection({ repos, initialBranch });
  useEffect(() => onChange(selection), [onChange, selection]);
  return null;
}

let container: HTMLDivElement;
let root: Root;
let queryClient: QueryClient;

async function renderSelection(
  repository: Repo,
  initialBranch?: string
): Promise<() => Selection> {
  let latest: Selection | undefined;
  const onChange = (selection: Selection) => {
    latest = selection;
  };

  await act(async () => {
    root.render(
      <QueryClientProvider client={queryClient}>
        <Probe
          repos={[repository]}
          initialBranch={initialBranch}
          onChange={onChange}
        />
      </QueryClientProvider>
    );
  });
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });

  return () => {
    if (!latest) throw new Error('hook did not render');
    return latest;
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
  queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

describe('useRepoBranchSelection', () => {
  it('emits origin/main instead of the registered checkout current branch', async () => {
    vi.mocked(repoApi.getBranches).mockResolvedValue([
      branch('vibe-kanban-deploy', { current: true }),
      branch('origin/main', { remote: true }),
    ]);

    const current = await renderSelection(repo());

    expect(current().isLoading).toBe(false);
    expect(current().configs[0]?.targetBranch).toBe('origin/main');
    expect(current().getWorkspaceRepoInputs()).toEqual([
      { repo_id: 'repo-1', target_branch: 'origin/main' },
    ]);
  });

  it('keeps a valid configured default ahead of origin/main', async () => {
    vi.mocked(repoApi.getBranches).mockResolvedValue([
      branch('local-current', { current: true }),
      branch('origin/main', { remote: true }),
      branch('origin/release', { remote: true }),
    ]);

    const current = await renderSelection(repo('origin/release'));

    expect(current().configs[0]?.targetBranch).toBe('origin/release');
  });

  it('preserves explicit initial and user override precedence', async () => {
    vi.mocked(repoApi.getBranches).mockResolvedValue([
      branch('local-current', { current: true }),
      branch('origin/main', { remote: true }),
      branch('origin/release', { remote: true }),
      branch('initial', { remote: true }),
    ]);

    const current = await renderSelection(repo('origin/release'), 'initial');
    expect(current().configs[0]?.targetBranch).toBe('initial');

    act(() => current().setRepoBranch('repo-1', 'origin/release'));
    expect(current().configs[0]?.targetBranch).toBe('origin/release');
  });
});
