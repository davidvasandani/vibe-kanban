/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RepoBranchStatus, RepoWithTargetBranch } from 'shared/types';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;

let branchStatus: RepoBranchStatus[] | undefined;

vi.mock('@/shared/hooks/useBranchStatus', () => ({
  useBranchStatus: () => ({ data: branchStatus }),
}));

import {
  deriveGitBehindHeaderStatus,
  GitBehindHeader,
} from './GitBehindHeader';

const repo = (
  id: string,
  name: string
): Pick<RepoWithTargetBranch, 'id' | 'name' | 'display_name'> => ({
  id,
  name,
  display_name: name,
});

const status = (
  repoId: string,
  commitsBehind: number | null
): Pick<RepoBranchStatus, 'repo_id' | 'commits_behind'> => ({
  repo_id: repoId,
  commits_behind: commitsBehind,
});

describe('deriveGitBehindHeaderStatus', () => {
  it('omits unavailable, null, and zero values', () => {
    expect(deriveGitBehindHeaderStatus([repo('one', 'web')], undefined)).toBe(
      null
    );
    expect(
      deriveGitBehindHeaderStatus([repo('one', 'web')], [status('one', null)])
    ).toBe(null);
    expect(
      deriveGitBehindHeaderStatus([repo('one', 'web')], [status('one', 0)])
    ).toBe(null);
  });

  it('uses compact single-repository copy and pluralizes accessible copy', () => {
    expect(
      deriveGitBehindHeaderStatus([repo('one', 'web')], [status('one', 3)])
    ).toEqual({
      visibleText: '3 behind',
      accessibleText: 'web is 3 commits behind',
    });

    expect(
      deriveGitBehindHeaderStatus([repo('one', 'web')], [status('one', 1)])
        ?.accessibleText
    ).toBe('web is 1 commit behind');
  });

  it('names behind repositories in repo order and joins status by ID', () => {
    expect(
      deriveGitBehindHeaderStatus(
        [repo('web-id', 'web'), repo('server-id', 'server')],
        [status('server-id', 5), status('web-id', 2)]
      )
    ).toEqual({
      visibleText: 'web 2 · server 5',
      accessibleText: 'web is 2 commits behind; server is 5 commits behind',
    });
  });

  it('keeps the repo name when only one of several repos is behind', () => {
    expect(
      deriveGitBehindHeaderStatus(
        [repo('web-id', 'web'), repo('server-id', 'server')],
        [status('web-id', 0), status('server-id', 1)]
      )
    ).toEqual({
      visibleText: 'server 1',
      accessibleText: 'server is 1 commit behind',
    });
  });
});

describe('GitBehindHeader', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    branchStatus = undefined;
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it('renders bounded visible and accessible status', () => {
    branchStatus = [status('one', 4) as RepoBranchStatus];

    act(() => {
      root.render(
        <GitBehindHeader
          workspaceId="workspace-id"
          repos={[repo('one', 'web') as RepoWithTargetBranch]}
        />
      );
    });

    const indicator = container.querySelector('span');
    expect(indicator?.textContent).toBe('4 behind');
    expect(indicator?.getAttribute('aria-label')).toBe(
      'web is 4 commits behind'
    );
    expect(indicator?.getAttribute('title')).toBe('web is 4 commits behind');
    expect(indicator?.classList).toContain('truncate');
    expect(indicator?.classList).toContain('max-w-40');
  });
});
