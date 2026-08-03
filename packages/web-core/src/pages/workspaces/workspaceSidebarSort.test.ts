import { describe, expect, it } from 'vitest';
import type { Workspace } from '@/shared/hooks/useWorkspaces';
import type {
  WorkspaceSortBy,
  WorkspaceSortOrder,
} from '@/shared/stores/useUiPreferencesStore';
import { sortWorkspaces, toSortKey } from './workspaceSidebarSort';

/**
 * Verbatim copy of the comparator this module replaced
 * (`WorkspacesSidebarContainer.tsx`, pre-`workspaceSidebarSort`). Kept here so
 * the new implementation is asserted to produce *identical* ordering rather than
 * merely plausible ordering — the whole change is a performance rewrite, so any
 * behaviour difference is a bug.
 */
function toTimestampLegacy(value: string | undefined): number | null {
  if (!value) {
    return null;
  }
  const timestamp = new Date(value).getTime();
  return Number.isNaN(timestamp) ? null : timestamp;
}

function getWorkspaceSortTimestampLegacy(
  workspace: Workspace,
  sortBy: WorkspaceSortBy
): number | null {
  if (sortBy === 'updated_at') {
    return toTimestampLegacy(workspace.latestProcessCompletedAt);
  }
  return toTimestampLegacy(workspace.createdAt);
}

function sortWorkspacesLegacy(
  workspaces: Workspace[],
  sortBy: WorkspaceSortBy,
  sortOrder: WorkspaceSortOrder
): Workspace[] {
  return [...workspaces].sort((a, b) => {
    if (a.isPinned !== b.isPinned) {
      return a.isPinned ? -1 : 1;
    }

    const aTimestamp = getWorkspaceSortTimestampLegacy(a, sortBy);
    const bTimestamp = getWorkspaceSortTimestampLegacy(b, sortBy);

    if (aTimestamp === null && bTimestamp === null) {
      return a.name.localeCompare(b.name);
    }
    if (aTimestamp === null) {
      return -1;
    }
    if (bTimestamp === null) {
      return 1;
    }

    if (aTimestamp === bTimestamp) {
      return a.name.localeCompare(b.name);
    }

    return sortOrder === 'asc'
      ? aTimestamp - bTimestamp
      : bTimestamp - aTimestamp;
  });
}

/**
 * `isPinned` is always an explicit boolean here because that is what production
 * data looks like: `toSidebarWorkspace` sets `isPinned: ws.pinned` and the wire
 * type declares `pinned: boolean`. See the "documented divergence" test below for
 * why that matters.
 */
function ws(overrides: Partial<Workspace> & { id: string }): Workspace {
  return {
    name: overrides.id,
    branch: `vk/${overrides.id}`,
    createdAt: '2026-01-01T00:00:00.000Z',
    updatedAt: '2026-01-01T00:00:00.000Z',
    description: '',
    isPinned: false,
    ...overrides,
  };
}

/**
 * Deliberately covers every branch of the comparator: pinned/unpinned, absent
 * timestamps (which is the *common* case before the first summaries response
 * lands, so every pair falls through to `localeCompare`), equal timestamps,
 * unparseable timestamps, and both sort fields.
 */
const FIXTURE: Workspace[] = [
  ws({
    id: 'b-pinned-recent',
    isPinned: true,
    latestProcessCompletedAt: '2026-03-02T10:00:00.000Z',
    createdAt: '2026-02-01T00:00:00.000Z',
  }),
  ws({
    id: 'a-pinned-recent',
    isPinned: true,
    latestProcessCompletedAt: '2026-03-02T10:00:00.000Z',
    createdAt: '2026-02-02T00:00:00.000Z',
  }),
  ws({
    id: 'pinned-no-ts',
    isPinned: true,
    createdAt: '2026-02-03T00:00:00.000Z',
  }),
  ws({ id: 'zebra-no-ts', createdAt: '2026-01-05T00:00:00.000Z' }),
  ws({ id: 'apple-no-ts', createdAt: '2026-01-06T00:00:00.000Z' }),
  ws({ id: 'Banana-no-ts', createdAt: '2026-01-07T00:00:00.000Z' }),
  ws({
    id: 'oldest',
    latestProcessCompletedAt: '2026-01-01T00:00:00.000Z',
    createdAt: '2026-01-01T00:00:00.000Z',
  }),
  ws({
    id: 'newest',
    latestProcessCompletedAt: '2026-06-01T00:00:00.000Z',
    createdAt: '2026-06-01T00:00:00.000Z',
  }),
  ws({
    id: 'tie-a',
    latestProcessCompletedAt: '2026-04-01T00:00:00.000Z',
    createdAt: '2026-04-01T00:00:00.000Z',
  }),
  ws({
    id: 'tie-b',
    latestProcessCompletedAt: '2026-04-01T00:00:00.000Z',
    createdAt: '2026-04-01T00:00:00.000Z',
  }),
  ws({
    id: 'bad-ts',
    latestProcessCompletedAt: 'not-a-date',
    createdAt: 'also-not-a-date',
  }),
  ws({
    id: 'explicitly-unpinned',
    isPinned: false,
    latestProcessCompletedAt: '2026-05-01T00:00:00.000Z',
    createdAt: '2026-05-01T00:00:00.000Z',
  }),
];

describe('documented divergence from the legacy comparator', () => {
  /**
   * The legacy comparator branched on `a.isPinned !== b.isPinned` with no
   * normalisation, so `isPinned: false` and `isPinned: undefined` compared as
   * *different* pin states and the `false` row was ordered ahead of the
   * `undefined` one regardless of timestamp. `toSortKey` normalises both to
   * `false`, so they compare by timestamp like any other pair.
   *
   * This cannot occur with real data — `toSidebarWorkspace` always sets
   * `isPinned` from `pinned: boolean` — but it is the one input for which the two
   * implementations disagree, so it is pinned down here rather than left as an
   * unexplained fixture omission.
   */
  it('treats undefined and false isPinned as the same pin state', () => {
    const undefinedPin: Workspace = {
      ...ws({ id: 'undefined-pin' }),
      isPinned: undefined,
      latestProcessCompletedAt: '2026-06-01T00:00:00.000Z',
    };
    const falsePin = ws({
      id: 'false-pin',
      isPinned: false,
      latestProcessCompletedAt: '2026-05-01T00:00:00.000Z',
    });

    // Newest first, i.e. ordered by timestamp and not by the pin-state accident.
    expect(
      sortWorkspaces([falsePin, undefinedPin], 'updated_at', 'desc').map(
        (w) => w.id
      )
    ).toEqual(['undefined-pin', 'false-pin']);

    // The legacy comparator put the explicitly-false row first instead.
    expect(
      sortWorkspacesLegacy([falsePin, undefinedPin], 'updated_at', 'desc').map(
        (w) => w.id
      )
    ).toEqual(['false-pin', 'undefined-pin']);
  });
});

const SORT_BYS: WorkspaceSortBy[] = ['updated_at', 'created_at'];
const SORT_ORDERS: WorkspaceSortOrder[] = ['asc', 'desc'];

describe('sortWorkspaces', () => {
  for (const sortBy of SORT_BYS) {
    for (const sortOrder of SORT_ORDERS) {
      it(`matches the legacy comparator for ${sortBy}/${sortOrder}`, () => {
        const actual = sortWorkspaces(FIXTURE, sortBy, sortOrder).map(
          (w) => w.id
        );
        const expected = sortWorkspacesLegacy(FIXTURE, sortBy, sortOrder).map(
          (w) => w.id
        );
        expect(actual).toEqual(expected);
      });
    }
  }

  it('does not mutate its input', () => {
    const original = FIXTURE.map((w) => w.id);
    sortWorkspaces(FIXTURE, 'updated_at', 'desc');
    expect(FIXTURE.map((w) => w.id)).toEqual(original);
  });

  it('keeps pinned workspaces ahead of every unpinned one', () => {
    const sorted = sortWorkspaces(FIXTURE, 'updated_at', 'desc');
    const lastPinned = sorted.findIndex((w) => w.isPinned !== true);
    expect(lastPinned).toBe(3);
    expect(sorted.slice(0, 3).every((w) => w.isPinned === true)).toBe(true);
  });

  it('handles an empty list', () => {
    expect(sortWorkspaces([], 'updated_at', 'desc')).toEqual([]);
  });
});

describe('toSortKey', () => {
  it('reads latestProcessCompletedAt for updated_at and createdAt otherwise', () => {
    const workspace = ws({
      id: 'x',
      latestProcessCompletedAt: '2026-03-02T10:00:00.000Z',
      createdAt: '2026-02-01T00:00:00.000Z',
    });
    expect(toSortKey(workspace, 'updated_at').ts).toBe(
      Date.parse('2026-03-02T10:00:00.000Z')
    );
    expect(toSortKey(workspace, 'created_at').ts).toBe(
      Date.parse('2026-02-01T00:00:00.000Z')
    );
  });

  it('reports null for missing and unparseable timestamps', () => {
    expect(toSortKey(ws({ id: 'x' }), 'updated_at').ts).toBeNull();
    expect(
      toSortKey(ws({ id: 'x', latestProcessCompletedAt: 'nope' }), 'updated_at')
        .ts
    ).toBeNull();
  });

  it('treats an absent isPinned as unpinned', () => {
    expect(toSortKey(ws({ id: 'x' }), 'updated_at').pinned).toBe(false);
  });
});
