import { afterEach, describe, expect, it, vi } from 'vitest';
import { getIssue, getProject } from './remoteApi';
import { getAuthRuntime } from '@/shared/lib/auth/runtime';

vi.mock('@/shared/lib/auth/runtime', () => ({
  getAuthRuntime: vi.fn(),
}));

describe('getIssue', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('returns the issue from the authoritative detail endpoint', async () => {
    vi.stubGlobal('__APP_VERSION__', 'test');
    vi.mocked(getAuthRuntime).mockReturnValue({
      getToken: vi.fn().mockResolvedValue('token'),
    } as never);
    const issue = { id: 'issue-1', simple_id: 'VK-123' };
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify(issue), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      )
    );

    await expect(getIssue('issue-1')).resolves.toMatchObject(issue);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('/v1/issues/issue-1'),
      expect.objectContaining({ method: 'GET' })
    );
  });

  it('returns null only when the issue detail endpoint confirms a miss', async () => {
    vi.stubGlobal('__APP_VERSION__', 'test');
    vi.mocked(getAuthRuntime).mockReturnValue({
      getToken: vi.fn().mockResolvedValue('token'),
    } as never);
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 404 }))
    );

    await expect(getIssue('missing-issue')).resolves.toBeNull();
  });
});

describe('getProject', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('returns the project from the authoritative detail endpoint', async () => {
    vi.stubGlobal('__APP_VERSION__', 'test');
    vi.mocked(getAuthRuntime).mockReturnValue({
      getToken: vi.fn().mockResolvedValue('token'),
    } as never);
    const project = { id: 'project-1', name: 'Project Alpha' };
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify(project), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        })
      )
    );

    await expect(getProject('project-1')).resolves.toMatchObject(project);
    expect(fetch).toHaveBeenCalledWith(
      expect.stringContaining('/v1/projects/project-1'),
      expect.objectContaining({ method: 'GET' })
    );
  });

  it('returns null only when the project detail endpoint confirms a miss', async () => {
    vi.stubGlobal('__APP_VERSION__', 'test');
    vi.mocked(getAuthRuntime).mockReturnValue({
      getToken: vi.fn().mockResolvedValue('token'),
    } as never);
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 404 }))
    );

    await expect(getProject('missing-project')).resolves.toBeNull();
  });
});
