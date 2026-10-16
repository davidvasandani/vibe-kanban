import { useMemo, useState } from 'react';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { ScratchType, type Scratch } from 'shared/types';
import { useScratch } from '@/shared/hooks/useScratch';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { localStorageScratchUpdate } from '@/shared/hooks/useLocalStorageScratch';
import type { KanbanProjectViewPreferences } from '@/shared/stores/useUiPreferencesStore';
import {
  SITE_BOARD_DEFAULTS_ID,
  boardStorageKey,
  emptyBoardDefaults,
  persistentBoardPreferences,
  readBoardDefault,
  readRememberedBoard,
  resolveBoardPreferences,
} from '../kanbanPreferences';

function useDefaultRecord(id: string, enabled: boolean) {
  const hostId = useHostId();
  const scope = `${hostId ?? 'local'}:${id}`;
  const record = useScratch(ScratchType.UI_PREFERENCES, id, { enabled });
  const [snapshot, setSnapshot] = useState<{
    scope: string;
    scratch: Scratch;
  } | null>(null);
  const saved = snapshot?.scope === scope ? snapshot.scratch : null;
  const setSaved = (scratch: Scratch) => setSnapshot({ scope, scratch });
  // A successful response is authoritative until the stream catches up.
  const scratch =
    saved?.id === id &&
    (!record.scratch || saved.updated_at > record.scratch.updated_at)
      ? saved
      : record.scratch;
  const data =
    enabled && scratch?.id === id && scratch.payload?.type === 'UI_PREFERENCES'
      ? scratch.payload.data
      : undefined;
  return { ...record, data, setSaved };
}

export function useKanbanPreferences(projectId: string, userId: string | null) {
  const runtime = useAppRuntime();
  const hostId = useHostId();
  const storageUser = `${hostId ?? 'local'}:${userId ?? 'anonymous'}`;
  const selectionKey = boardStorageKey(storageUser, projectId, 'selection');
  const [selection, setSelection] = useState({ key: '', viewId: 'team' });
  const storedView = useMemo(() => {
    try {
      return localStorage.getItem(selectionKey) === 'personal'
        ? 'personal'
        : 'team';
    } catch {
      return 'team';
    }
  }, [selectionKey]);
  const activeViewId =
    selection.key === selectionKey ? selection.viewId : storedView;
  const key = boardStorageKey(storageUser, projectId, activeViewId);
  const [edited, setEdited] = useState<{
    key: string;
    value: KanbanProjectViewPreferences | undefined;
  }>({ key: '', value: undefined });
  const [notice, setNotice] = useState<{
    key: string;
    error: string | null;
    message: string | null;
  }>({ key: '', error: null, message: null });
  const [saving, setSaving] = useState(false);
  const [search, setSearchState] = useState({ key: '', query: '' });
  const personal = useDefaultRecord(userId ?? '', !!userId);
  const site = useDefaultRecord(SITE_BOARD_DEFAULTS_ID, runtime === 'local');
  const personalDefault = useMemo(
    () => readBoardDefault(personal.data, activeViewId),
    [personal.data, activeViewId]
  );
  const siteDefault = useMemo(
    () => readBoardDefault(site.data, activeViewId),
    [site.data, activeViewId]
  );
  const storedPreferences = useMemo(() => readRememberedBoard(key), [key]);
  const remembered = edited.key === key ? edited.value : storedPreferences;
  const defaults = useMemo(
    () =>
      resolveBoardPreferences(
        projectId,
        activeViewId,
        undefined,
        personalDefault,
        siteDefault
      ),
    [projectId, activeViewId, personalDefault, siteDefault]
  );
  const basePreferences = remembered ?? defaults;
  const query = search.key === key ? search.query : '';
  const preferences = useMemo(
    () =>
      query
        ? {
            ...basePreferences,
            filters: { ...basePreferences.filters, searchQuery: query },
          }
        : basePreferences,
    [basePreferences, query]
  );
  function setSearch(query: string) {
    setSearchState({ key, query });
  }

  const ready =
    (!userId || (!personal.isLoading && !personal.error)) &&
    (runtime !== 'local' || (!site.isLoading && !site.error));

  function update(value: KanbanProjectViewPreferences | undefined) {
    setEdited({
      key,
      value: value ? persistentBoardPreferences(value) : undefined,
    });
    if (!value) setSearch('');
    try {
      if (value)
        localStorage.setItem(
          key,
          JSON.stringify(persistentBoardPreferences(value))
        );
      else localStorage.removeItem(key);
      setNotice({ key, error: null, message: null });
    } catch {
      setNotice({
        key,
        error: 'Could not remember these filters in this browser.',
        message: null,
      });
    }
  }

  async function saveDefault(scope: 'personal' | 'site', clear = false) {
    if (
      !ready ||
      saving ||
      (scope === 'personal' && !userId) ||
      (scope === 'site' && runtime !== 'local')
    )
      return;
    const record = scope === 'personal' ? personal : site;
    const id = scope === 'personal' ? userId! : SITE_BOARD_DEFAULTS_ID;
    const data = { ...emptyBoardDefaults(), ...record.data };
    const entries = { ...data.kanban_project_view_preferences };
    if (clear) delete entries[activeViewId];
    else
      entries[activeViewId] = {
        projectId,
        preferences: persistentBoardPreferences(preferences),
      };
    const payload = {
      type: 'UI_PREFERENCES' as const,
      data: { ...data, kanban_project_view_preferences: entries },
    };
    setSaving(true);
    setNotice({ key, error: null, message: null });
    try {
      if (runtime === 'remote') {
        if (
          !localStorageScratchUpdate(ScratchType.UI_PREFERENCES, id, {
            payload,
          })
        )
          throw new Error('Storage unavailable');
      } else {
        record.setSaved(await record.updateScratch({ payload }));
      }
      setNotice({
        key,
        error: null,
        message: clear ? 'Default removed.' : 'Default saved.',
      });
    } catch {
      setNotice({
        key,
        error: 'Could not save the default. Please try again.',
        message: null,
      });
    } finally {
      setSaving(false);
    }
  }

  function setView(viewId: string) {
    setSelection({ key: selectionKey, viewId });
    try {
      localStorage.setItem(selectionKey, viewId);
    } catch {
      setNotice({
        key,
        error: 'Could not remember the selected view.',
        message: null,
      });
    }
  }

  return {
    activeViewId,
    preferences,
    defaults,
    update,
    setSearch,
    setView,
    saveDefault,
    saving,
    ready,
    loading:
      (!!userId && personal.isLoading) ||
      (runtime === 'local' && site.isLoading),
    hasPersonalDefault: !!personalDefault,
    hasSiteDefault: !!siteDefault,
    canSavePersonal: !!userId,
    canSaveSite: runtime === 'local',
    browserOnlyDefaults: runtime === 'remote',
    error:
      (notice.key === key ? notice.error : null) ??
      personal.error ??
      site.error,
    message: notice.key === key ? notice.message : null,
  };
}
