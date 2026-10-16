/* @vitest-environment jsdom */
import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { KanbanFiltersDialog } from './KanbanFiltersDialog';
import { DEFAULT_KANBAN_FILTER_STATE } from '@/shared/stores/useUiPreferencesStore';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (_key: string, fallback: string) => fallback }),
}));
vi.mock('@/shared/dialogs/kanban/AssigneeSelectionDialog', () => ({
  AssigneeSelectionDialog: { show: vi.fn() },
}));
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

describe('board filter default actions', () => {
  let root: Root;
  let container: HTMLDivElement;
  const saveDefault = vi.fn(async () => {});
  const update = vi.fn();
  beforeEach(() => {
    saveDefault.mockClear();
    update.mockClear();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });
  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });
  function render(overrides = {}) {
    act(() =>
      root.render(
        <KanbanFiltersDialog
          open
          onOpenChange={() => {}}
          projectId="p1"
          currentUserId="u1"
          tags={[]}
          users={[]}
          filters={DEFAULT_KANBAN_FILTER_STATE}
          showSubIssues={false}
          showWorkspaces
          hideBlocked={false}
          onPrioritiesChange={() => {}}
          onAssigneesChange={() => {}}
          onTagsChange={() => {}}
          onSortChange={() => {}}
          onShowSubIssuesChange={() => {}}
          onShowWorkspacesChange={() => {}}
          onHideBlockedChange={() => {}}
          preferences={{
            ready: true,
            saving: false,
            canSavePersonal: true,
            canSaveSite: true,
            hasPersonalDefault: true,
            hasSiteDefault: true,
            browserOnlyDefaults: false,
            error: null,
            message: null,
            saveDefault,
            update,
            ...overrides,
          }}
        />
      )
    );
  }
  function button(label: string) {
    return [...document.querySelectorAll('button')].find(
      (item) => item.textContent === label
    )!;
  }
  it('provides personal/site save and restore actions in the dialog', () => {
    render();
    act(() => button('Save as my default').click());
    expect(saveDefault).toHaveBeenCalledWith('personal');
    act(() => button('Save site-wide default').click());
    expect(saveDefault).toHaveBeenCalledWith('site');
    act(() => button('Use saved defaults').click());
    expect(update).toHaveBeenCalledWith(undefined);
  });
  it('disables saves while pending and displays failures accessibly', () => {
    render({ saving: true, error: 'Could not save the default.' });
    expect(button('Save site-wide default').disabled).toBe(true);
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      'Could not save'
    );
    act(() => button('Save site-wide default').click());
    expect(saveDefault).not.toHaveBeenCalled();
  });
  it('does not offer site-wide writes for browser-only storage', () => {
    render({ canSaveSite: false, browserOnlyDefaults: true });
    expect(button('Save site-wide default')).toBeUndefined();
    expect(document.body.textContent).toContain(
      'personal defaults are saved in this browser only'
    );
  });
});
