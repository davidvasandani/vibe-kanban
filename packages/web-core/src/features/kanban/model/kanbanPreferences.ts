import { z } from 'zod';
import type { UiPreferencesData } from 'shared/types';
import {
  resolveKanbanProjectState,
  type KanbanProjectViewPreferences,
} from '@/shared/stores/useUiPreferencesStore';

export const SITE_BOARD_DEFAULTS_ID = '00000000-0000-0000-0000-000000000002';

export const boardPreferencesSchema = z.object({
  filters: z.object({
    searchQuery: z.string(),
    priorities: z.array(z.enum(['urgent', 'high', 'medium', 'low'])),
    assigneeIds: z.array(z.string()),
    tagIds: z.array(z.string()),
    sortField: z.enum([
      'sort_order',
      'priority',
      'created_at',
      'updated_at',
      'title',
    ]),
    sortDirection: z.enum(['asc', 'desc']),
  }),
  showSubIssues: z.boolean(),
  showWorkspaces: z.boolean(),
  hideBlocked: z.boolean(),
});

export const boardDefaultSchema = z.object({
  projectId: z.string(),
  preferences: boardPreferencesSchema,
});
export type BoardDefault = z.infer<typeof boardDefaultSchema>;

export function persistentBoardPreferences(
  preferences: KanbanProjectViewPreferences
) {
  return boardPreferencesSchema.parse({
    ...preferences,
    filters: { ...preferences.filters, searchQuery: '' },
  });
}

export function resolveBoardPreferences(
  projectId: string,
  viewId: string,
  remembered?: KanbanProjectViewPreferences,
  personal?: BoardDefault,
  site?: BoardDefault
): KanbanProjectViewPreferences {
  if (remembered) return remembered;
  const saved = personal ?? site;
  if (!saved)
    return persistentBoardPreferences(
      resolveKanbanProjectState({ activeViewId: viewId })
    );
  const preferences = persistentBoardPreferences(saved.preferences);
  if (saved.projectId !== projectId) preferences.filters.tagIds = [];
  return preferences;
}

export function boardStorageKey(
  userId: string | null,
  projectId: string,
  viewId: string
) {
  return `vk-board-preferences:v1:${userId ?? 'anonymous'}:${projectId}:${viewId}`;
}

export function readRememberedBoard(key: string) {
  try {
    const parsed = boardPreferencesSchema.safeParse(
      JSON.parse(localStorage.getItem(key) ?? 'null')
    );
    return parsed.success ? persistentBoardPreferences(parsed.data) : undefined;
  } catch {
    return undefined;
  }
}

export function readBoardDefault(data: unknown, viewId: string) {
  const record = z
    .object({ kanban_project_view_preferences: z.record(z.unknown()) })
    .safeParse(data);
  if (!record.success) return undefined;
  const parsed = boardDefaultSchema.safeParse(
    record.data.kanban_project_view_preferences[viewId]
  );
  return parsed.success ? parsed.data : undefined;
}

// These records are dedicated to board defaults, never the legacy UI scratch.
export function emptyBoardDefaults(): UiPreferencesData {
  return {
    repo_actions: {},
    expanded: {},
    context_bar_position: null,
    pane_sizes: {},
    collapsed_paths: {},
    file_search_repo_id: null,
    is_left_sidebar_visible: null,
    is_right_sidebar_visible: null,
    is_terminal_visible: null,
    workspace_panel_states: {},
    workspace_filters: {
      project_ids: [],
      pr_filter: 'all',
      hidden_issue_status_names: [],
    },
    workspace_sort: { sort_by: 'updated_at', sort_order: 'desc' },
    selected_org_id: null,
    selected_project_id: null,
    create_draft_workspace_by_default: null,
    kanban_project_view_selections: {},
    kanban_project_view_preferences: {},
    kanban_view_mode: null,
    list_view_status_filter_name: null,
  };
}
