/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Scratch, UpdateScratch } from 'shared/types';
import { useKanbanPreferences } from './useKanbanPreferences';
import {
  emptyBoardDefaults,
  SITE_BOARD_DEFAULTS_ID,
  resolveBoardPreferences,
} from '../kanbanPreferences';

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const state = vi.hoisted(() => ({
  records: new Map<string, Scratch>(),
  loading: false,
  runtime: 'local',
  host: 'host-a',
  update: vi.fn(),
}));
vi.mock('@/shared/hooks/useAppRuntime', () => ({
  useAppRuntime: () => state.runtime,
}));
vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => state.host,
}));
vi.mock('@/shared/hooks/useScratch', () => ({
  useScratch: (_type: string, id: string, options: { enabled: boolean }) => ({
    scratch: options.enabled ? (state.records.get(id) ?? null) : null,
    isLoading: options.enabled && state.loading,
    isConnected: true,
    error: null,
    updateScratch: (update: UpdateScratch) => state.update(id, update),
  }),
}));
let hook: ReturnType<typeof useKanbanPreferences>;
function Harness({
  user = 'u1',
  project = 'p1',
}: {
  user?: string;
  project?: string;
}) {
  hook = useKanbanPreferences(project, user);
  return <output>{hook.preferences.filters.sortField}</output>;
}
function scratch(id: string, update: UpdateScratch): Scratch {
  return {
    id,
    payload: update.payload,
    created_at: '2026-10-16T00:00:00Z',
    updated_at: '2026-10-16T00:00:00Z',
  };
}

describe('remembered filters and explicit defaults', () => {
  let root: Root;
  let container: HTMLDivElement;
  beforeEach(() => {
    localStorage.clear();
    state.records.clear();
    state.loading = false;
    state.runtime = 'local';
    state.host = 'host-a';
    state.update
      .mockReset()
      .mockImplementation(async (id: string, update: UpdateScratch) =>
        scratch(id, update)
      );
    container = document.createElement('div');
    root = createRoot(container);
    act(() => root.render(<Harness />));
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    vi.restoreAllMocks();
  });
  it('keeps resolved filter references stable on unrelated renders', () => {
    const previous = hook.preferences;
    act(() => root.render(<Harness />));
    expect(hook.preferences).toBe(previous);
  });
  it('restores all settings on remount, excludes search, and isolates users and projects', () => {
    act(() =>
      hook.update({
        filters: {
          searchQuery: 'query',
          priorities: ['low'],
          assigneeIds: ['__self__'],
          tagIds: ['t'],
          sortField: 'updated_at',
          sortDirection: 'desc',
        },
        showSubIssues: true,
        showWorkspaces: false,
        hideBlocked: true,
      })
    );
    act(() => root.render(<Harness user="u2" />));
    expect(hook.preferences.filters.sortField).toBe('sort_order');
    act(() => root.render(<Harness project="p2" />));
    expect(hook.preferences.hideBlocked).toBe(false);
    act(() => root.unmount());
    root = createRoot(container);
    act(() => root.render(<Harness />));
    expect(hook.preferences).toEqual({
      filters: {
        searchQuery: '',
        priorities: ['low'],
        assigneeIds: ['__self__'],
        tagIds: ['t'],
        sortField: 'updated_at',
        sortDirection: 'desc',
      },
      showSubIssues: true,
      showWorkspaces: false,
      hideBlocked: true,
    });
  });
  it('remembers Team/Personal selection independently per user', () => {
    act(() => hook.setView('personal'));
    act(() => root.render(<Harness user="u2" />));
    expect(hook.activeViewId).toBe('team');
    act(() => root.render(<Harness />));
    expect(hook.activeViewId).toBe('personal');
  });
  it('waits for initialization before saving, then persists a personal default separately from site', async () => {
    state.loading = true;
    act(() => root.render(<Harness />));
    await act(() => hook.saveDefault('personal'));
    expect(state.update).not.toHaveBeenCalled();
    state.loading = false;
    act(() => root.render(<Harness />));
    act(() => hook.update({ ...hook.preferences, hideBlocked: true }));
    await act(() => hook.saveDefault('personal'));
    expect(state.update.mock.calls[0][0]).toBe('u1');
    expect(hook.message).toBe('Default saved.');
    act(() => hook.update(undefined));
    expect(hook.preferences.hideBlocked).toBe(true);
    await act(() => hook.saveDefault('personal', true));
    expect(hook.preferences.hideBlocked).toBe(false);
  });
  it('uses site defaults, retains edits across late data, and restores defaults explicitly', () => {
    act(() => hook.update({ ...hook.preferences, showWorkspaces: false }));
    const data = emptyBoardDefaults();
    data.kanban_project_view_preferences.team = {
      projectId: 'p1',
      preferences: {
        ...resolveBoardPreferences('p1', 'team'),
        hideBlocked: true,
      },
    };
    state.records.set(
      SITE_BOARD_DEFAULTS_ID,
      scratch(SITE_BOARD_DEFAULTS_ID, {
        payload: { type: 'UI_PREFERENCES', data },
      })
    );
    act(() => root.render(<Harness />));
    expect(hook.preferences.hideBlocked).toBe(false);
    expect(hook.preferences.showWorkspaces).toBe(false);
    act(() => hook.update(undefined));
    expect(hook.preferences.hideBlocked).toBe(true);
  });
  it('does not pin inherited defaults when searching or clearing search', () => {
    act(() => hook.setSearch('find me'));
    expect(hook.preferences.filters.searchQuery).toBe('find me');
    expect(localStorage.length).toBe(0);
    act(() => hook.setSearch(''));
    const data = emptyBoardDefaults();
    data.kanban_project_view_preferences.team = {
      projectId: 'p1',
      preferences: {
        ...resolveBoardPreferences('p1', 'team'),
        hideBlocked: true,
      },
    };
    state.records.set(
      SITE_BOARD_DEFAULTS_ID,
      scratch(SITE_BOARD_DEFAULTS_ID, {
        payload: { type: 'UI_PREFERENCES', data },
      })
    );
    act(() => root.render(<Harness />));
    expect(hook.preferences.hideBlocked).toBe(true);
    expect(localStorage.length).toBe(0);
  });
  it('reports failed saves and unavailable browser storage', async () => {
    state.update.mockRejectedValue(new Error('offline'));
    await act(() => hook.saveDefault('site'));
    expect(hook.error).toContain('Could not save');
    expect(hook.message).toBeNull();
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    act(() => hook.update({ ...hook.preferences, hideBlocked: true }));
    expect(hook.error).toContain('Could not remember');
  });
  it('does not advertise browser storage as a shared site default', async () => {
    state.runtime = 'remote';
    act(() => root.render(<Harness />));
    expect(hook.canSaveSite).toBe(false);
    await act(() => hook.saveDefault('site'));
    expect(state.update).not.toHaveBeenCalled();
  });
});
