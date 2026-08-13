import { describe, expect, it } from 'vitest';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import { sortWorkspaces } from './workspaceSort';

function workspace(
  id: string,
  overrides: Partial<SidebarWorkspace> = {}
): SidebarWorkspace {
  return {
    id,
    name: id,
    branch: id,
    createdAt: '2026-08-01T00:00:00Z',
    updatedAt: '2026-08-01T00:00:00Z',
    description: '',
    ...overrides,
  };
}

function ids(workspaces: SidebarWorkspace[]): string[] {
  return workspaces.map(({ id }) => id);
}

describe('sortWorkspaces', () => {
  it('uses persisted update times while summaries are unavailable', () => {
    const older = workspace('older', {
      name: 'Alpha',
      updatedAt: '2026-08-01T00:00:00Z',
    });
    const newer = workspace('newer', {
      name: 'Zulu',
      updatedAt: '2026-08-02T00:00:00Z',
    });

    expect(ids(sortWorkspaces([older, newer], 'updated_at', 'desc'))).toEqual([
      'newer',
      'older',
    ]);
  });

  it('prefers a valid latest process completion time over updatedAt', () => {
    const recentlyMutated = workspace('recently-mutated', {
      updatedAt: '2026-08-03T00:00:00Z',
      latestProcessCompletedAt: '2026-08-01T00:00:00Z',
    });
    const recentlyCompleted = workspace('recently-completed', {
      updatedAt: '2026-07-01T00:00:00Z',
      latestProcessCompletedAt: '2026-08-02T00:00:00Z',
    });

    expect(
      ids(
        sortWorkspaces(
          [recentlyMutated, recentlyCompleted],
          'updated_at',
          'desc'
        )
      )
    ).toEqual(['recently-completed', 'recently-mutated']);
  });

  it('falls back to updatedAt when the process timestamp is invalid', () => {
    const fallback = workspace('fallback', {
      updatedAt: '2026-08-02T00:00:00Z',
      latestProcessCompletedAt: 'not-a-date',
    });
    const older = workspace('older', {
      updatedAt: '2026-08-01T00:00:00Z',
    });

    expect(
      ids(sortWorkspaces([older, fallback], 'updated_at', 'desc'))
    ).toEqual(['fallback', 'older']);
  });

  it('keeps missing selected timestamps last in either direction', () => {
    const missing = workspace('missing', { createdAt: 'invalid' });
    const earlier = workspace('earlier', {
      createdAt: '2026-08-01T00:00:00Z',
    });
    const later = workspace('later', {
      createdAt: '2026-08-02T00:00:00Z',
    });

    expect(
      ids(sortWorkspaces([missing, later, earlier], 'created_at', 'asc'))
    ).toEqual(['earlier', 'later', 'missing']);
    expect(
      ids(sortWorkspaces([missing, earlier, later], 'created_at', 'desc'))
    ).toEqual(['later', 'earlier', 'missing']);
  });

  it('keeps pinned workspaces first regardless of direction', () => {
    const pinned = workspace('pinned', {
      isPinned: true,
      createdAt: '2026-08-02T00:00:00Z',
    });
    const unpinned = workspace('unpinned', {
      createdAt: '2026-08-01T00:00:00Z',
    });

    expect(
      ids(sortWorkspaces([unpinned, pinned], 'created_at', 'asc'))
    ).toEqual(['pinned', 'unpinned']);
  });

  it('breaks timestamp ties by name and then workspace ID', () => {
    const second = workspace('workspace-b', { name: 'Same' });
    const first = workspace('workspace-a', { name: 'Same' });
    const namedFirst = workspace('workspace-c', { name: 'Alpha' });

    expect(
      ids(sortWorkspaces([second, namedFirst, first], 'updated_at', 'desc'))
    ).toEqual(['workspace-c', 'workspace-a', 'workspace-b']);
  });

  it('does not mutate its input', () => {
    const older = workspace('older', {
      updatedAt: '2026-08-01T00:00:00Z',
    });
    const newer = workspace('newer', {
      updatedAt: '2026-08-02T00:00:00Z',
    });
    const input = [older, newer];

    sortWorkspaces(input, 'updated_at', 'desc');

    expect(ids(input)).toEqual(['older', 'newer']);
  });
});
