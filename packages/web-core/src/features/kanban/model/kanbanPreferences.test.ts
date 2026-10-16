import { describe, expect, it } from 'vitest';
import {
  boardPreferencesSchema,
  persistentBoardPreferences,
  resolveBoardPreferences,
  boardStorageKey,
  readBoardDefault,
  emptyBoardDefaults,
} from './kanbanPreferences';

const base = resolveBoardPreferences('p1', 'team');
const site = {
  projectId: 'p1',
  preferences: {
    ...base,
    hideBlocked: true,
    filters: {
      ...base.filters,
      priorities: ['high' as const],
      tagIds: ['tag'],
      assigneeIds: ['__self__'],
    },
  },
};

describe('board preference resolution', () => {
  it('resolves remembered > personal > site > built-in, including empty and false values', () => {
    const personal = {
      projectId: 'p1',
      preferences: { ...base, showWorkspaces: false },
    };
    expect(resolveBoardPreferences('p1', 'team')).toMatchObject({
      hideBlocked: false,
      showSubIssues: false,
    });
    expect(
      resolveBoardPreferences('p1', 'personal').filters.assigneeIds
    ).toEqual(['__self__']);
    expect(
      resolveBoardPreferences('p1', 'team', undefined, undefined, site)
        .hideBlocked
    ).toBe(true);
    expect(
      resolveBoardPreferences('p1', 'team', undefined, personal, site)
    ).toEqual(personal.preferences);
    expect(resolveBoardPreferences('p1', 'team', base, personal, site)).toEqual(
      base
    );
  });
  it('keeps source-project tags, drops cross-project tags and preserves relative Me', () => {
    expect(
      resolveBoardPreferences('p1', 'team', undefined, site).filters.tagIds
    ).toEqual(['tag']);
    const next = resolveBoardPreferences('p2', 'team', undefined, site);
    expect(next.filters.tagIds).toEqual([]);
    expect(next.filters.assigneeIds).toEqual(['__self__']);
    expect(site.preferences.filters.tagIds).toEqual(['tag']);
  });
  it('strips transient search and clones arrays', () => {
    const saved = persistentBoardPreferences({
      ...site.preferences,
      filters: { ...site.preferences.filters, searchQuery: 'secret search' },
    });
    expect(saved.filters.searchQuery).toBe('');
    saved.filters.priorities.length = 0;
    expect(site.preferences.filters.priorities).toEqual(['high']);
  });
  it('rejects malformed persisted preferences', () => {
    expect(boardPreferencesSchema.safeParse({ filters: {} }).success).toBe(
      false
    );
    const data = emptyBoardDefaults();
    data.kanban_project_view_preferences.team = {
      projectId: 'p1',
      preferences: { hideBlocked: 'false' },
    };
    expect(readBoardDefault(data, 'team')).toBeUndefined();
  });
  it('falls back for absent, null and malformed default record envelopes', () => {
    for (const data of [
      undefined,
      null,
      {},
      { kanban_project_view_preferences: null },
      { kanban_project_view_preferences: 'bad' },
    ]) {
      expect(readBoardDefault(data, 'team')).toBeUndefined();
    }
  });
  it('isolates users, projects, views and anonymous choices', () => {
    const keys = [
      boardStorageKey('u1', 'p1', 'team'),
      boardStorageKey('u2', 'p1', 'team'),
      boardStorageKey(null, 'p1', 'team'),
      boardStorageKey('u1', 'p2', 'team'),
      boardStorageKey('u1', 'p1', 'personal'),
    ];
    expect(new Set(keys).size).toBe(keys.length);
  });
});
