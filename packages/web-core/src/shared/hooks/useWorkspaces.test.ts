import { describe, expect, it } from 'vitest';
import type { WorkspaceSummary, WorkspaceWithStatus } from 'shared/types';
import { toSidebarWorkspaces, type RowCache } from './useWorkspaces';

function wsRecord(id: string): WorkspaceWithStatus {
  return {
    id,
    task_id: null,
    container_ref: `/tmp/${id}`,
    branch: `vk/${id}`,
    setup_completed_at: null,
    created_at: '2026-01-01T00:00:00.000Z',
    updated_at: '2026-01-01T00:00:00.000Z',
    archived: false,
    pinned: false,
    name: id,
    worktree_deleted: false,
    current_pipeline_stage: null,
    speckit_feature_key: null,
    speckit_host_repo_id: null,
    is_running: false,
    is_errored: false,
  } as WorkspaceWithStatus;
}

function summary(
  workspaceId: string,
  filesChanged: number | null
): WorkspaceSummary {
  return {
    workspace_id: workspaceId,
    latest_session_id: null,
    has_pending_approval: false,
    files_changed: filesChanged,
    lines_added: filesChanged,
    lines_removed: 0,
    latest_process_status: null,
    has_running_dev_server: false,
    has_unseen_turns: false,
    pr_status: null,
    pr_number: null,
    pr_url: null,
  } as WorkspaceSummary;
}

describe('toSidebarWorkspaces', () => {
  /**
   * The sidebar re-derives its list on every WebSocket patch and every 15s
   * summaries poll. Before this cache, each pass allocated a fresh row object per
   * workspace, so element identity always changed, every downstream filter/sort
   * memo was invalidated, and `React.memo` on the row could never hit.
   */
  it('returns identical row references when inputs are unchanged', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const b = wsRecord('b');
    const byId = { a, b };
    const summaries = new Map([['a', summary('a', 3)]]);

    const first = toSidebarWorkspaces(byId, summaries, cache);
    const second = toSidebarWorkspaces(byId, summaries, cache);

    expect(second[0]).toBe(first[0]);
    expect(second[1]).toBe(first[1]);
  });

  it('rebuilds only the row whose workspace record changed', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const b = wsRecord('b');
    const summaries = new Map<string, WorkspaceSummary>();

    const first = toSidebarWorkspaces({ a, b }, summaries, cache);
    const changedA = { ...a, is_running: true };
    const second = toSidebarWorkspaces({ a: changedA, b }, summaries, cache);

    expect(second[0]).not.toBe(first[0]);
    expect(second[0].isRunning).toBe(true);
    expect(second[1]).toBe(first[1]);
  });

  it('rebuilds only the row whose summary changed', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const b = wsRecord('b');
    const byId = { a, b };

    const first = toSidebarWorkspaces(
      byId,
      new Map([['a', summary('a', 1)]]),
      cache
    );
    const second = toSidebarWorkspaces(
      byId,
      new Map([['a', summary('a', 9)]]),
      cache
    );

    expect(second[0]).not.toBe(first[0]);
    expect(second[0].filesChanged).toBe(9);
    expect(second[1]).toBe(first[1]);
  });

  it('surfaces a null files_changed as undefined, so the badge stays hidden', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const rows = toSidebarWorkspaces(
      { a },
      new Map([['a', summary('a', null)]]),
      cache
    );
    expect(rows[0].filesChanged).toBeUndefined();
  });

  /**
   * The summaries query rebuilds its `Map` and every `WorkspaceSummary` object on
   * every 15s poll — react-query does no structural sharing for a `Map`. An
   * identity-only check would therefore miss for every row on every poll, which
   * would rebuild every row and make `React.memo` on the row component useless
   * against the dominant cost this change targets.
   */
  it('reuses rows when an equal-valued but freshly-allocated summary arrives', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const byId = { a };

    const first = toSidebarWorkspaces(byId, new Map([['a', summary('a', 3)]]), cache);
    // A new poll: same values, brand-new Map and brand-new object.
    const second = toSidebarWorkspaces(byId, new Map([['a', summary('a', 3)]]), cache);

    expect(second[0]).toBe(first[0]);
  });

  it('still rebuilds when a freshly-allocated summary differs in value', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const byId = { a };

    const first = toSidebarWorkspaces(byId, new Map([['a', summary('a', 3)]]), cache);
    const second = toSidebarWorkspaces(byId, new Map([['a', summary('a', 4)]]), cache);

    expect(second[0]).not.toBe(first[0]);
    expect(second[0].filesChanged).toBe(4);
  });

  /**
   * Several consumers read this ordering positionally instead of re-sorting
   * (`WorkspaceSelectionDialog` paginates without sorting, `getNextWorkspaceId`
   * picks an index-adjacent workspace, `CreateModeProvider` takes the head), so
   * it is a contract rather than a convenience.
   */
  it('orders pinned first, then newest created_at', () => {
    const cache: RowCache = new Map();
    const old = { ...wsRecord('old'), created_at: '2026-01-01T00:00:00.000Z' };
    const recent = { ...wsRecord('recent'), created_at: '2026-06-01T00:00:00.000Z' };
    const pinnedOld = {
      ...wsRecord('pinned-old'),
      created_at: '2025-01-01T00:00:00.000Z',
      pinned: true,
    };

    const rows = toSidebarWorkspaces(
      { old, recent, 'pinned-old': pinnedOld },
      new Map(),
      cache
    );

    expect(rows.map((r) => r.id)).toEqual(['pinned-old', 'recent', 'old']);
  });

  it('prunes ids that leave the stream so the cache cannot grow unboundedly', () => {
    const cache: RowCache = new Map();
    const a = wsRecord('a');
    const b = wsRecord('b');
    const summaries = new Map<string, WorkspaceSummary>();

    toSidebarWorkspaces({ a, b }, summaries, cache);
    expect(cache.size).toBe(2);

    toSidebarWorkspaces({ a }, summaries, cache);
    expect(cache.size).toBe(1);
    expect(cache.has('b')).toBe(false);
  });

  it('clears the cache and returns an empty list before the stream initialises', () => {
    const cache: RowCache = new Map();
    toSidebarWorkspaces({ a: wsRecord('a') }, new Map(), cache);
    expect(cache.size).toBe(1);

    expect(toSidebarWorkspaces(undefined, new Map(), cache)).toEqual([]);
    expect(cache.size).toBe(0);
  });
});
