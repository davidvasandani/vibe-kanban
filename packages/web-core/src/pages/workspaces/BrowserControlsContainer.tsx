import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PlusIcon, XIcon } from '@phosphor-icons/react';
import type { BrowserController, BrowserSessionWithState } from 'shared/types';
import { browserSessionsApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import {
  useBrowserSessionStore,
  useSelectedBrowserSessionId,
} from '@/shared/stores/useBrowserSessionStore';
import { browserStatusDotClass } from './BrowserPanelContainer';

interface BrowserControlsContainerProps {
  workspaceId: string;
  className: string;
}

export function BrowserControlsContainer({
  workspaceId,
  className,
}: BrowserControlsContainerProps) {
  const { t } = useTranslation('tasks');
  const [sessions, setSessions] = useState<BrowserSessionWithState[]>([]);
  const [selectedSessionId, setSelectedSessionId] =
    useSelectedBrowserSessionId(workspaceId);
  const listRefreshNonce = useBrowserSessionStore((s) => s.listRefreshNonce);
  const [profile, setProfile] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await browserSessionsApi.list(workspaceId);
      setSessions(list);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [workspaceId]);

  // Poll the session list every 10s while mounted, and refetch when the
  // panel observes WS state changes (listRefreshNonce).
  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 10_000);
    return () => window.clearInterval(timer);
  }, [refresh, listRefreshNonce]);

  const handleCreate = useCallback(async () => {
    setIsCreating(true);
    try {
      const created = await browserSessionsApi.create({
        workspace_id: workspaceId,
        profile: profile.trim() === '' ? null : profile.trim(),
      });
      setProfile('');
      setSelectedSessionId(created.session.id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsCreating(false);
    }
  }, [workspaceId, profile, setSelectedSessionId, refresh]);

  const handleClose = useCallback(
    async (sessionId: string) => {
      try {
        try {
          await browserSessionsApi.close(sessionId);
        } catch (err) {
          // A live controller blocks a plain close (CONTROL_CONFLICT).
          // Closing anyway is an explicit, confirmed force.
          const message = err instanceof Error ? err.message : String(err);
          if (
            message.includes('CONTROL_CONFLICT') &&
            window.confirm(t('browser.session.forceCloseConfirm'))
          ) {
            await browserSessionsApi.close(sessionId, true);
          } else {
            throw err;
          }
        }
        if (selectedSessionId === sessionId) {
          setSelectedSessionId(null);
        }
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [selectedSessionId, setSelectedSessionId, refresh, t]
  );

  const controllerSummary = (controller: BrowserController | undefined) => {
    if (!controller || controller.type === 'none') {
      return t('browser.controller.none');
    }
    if (controller.type === 'agent') {
      return `${t('browser.controller.agent')} · ${controller.execution_id.slice(0, 8)}`;
    }
    return t('browser.controller.human');
  };

  return (
    <div className={cn('flex flex-col w-full min-h-0', className)}>
      {/* New session form */}
      <div className="flex items-center gap-half p-half border-b">
        <input
          value={profile}
          onChange={(e) => setProfile(e.target.value)}
          placeholder={t('browser.sessions.profilePlaceholder')}
          className="flex-1 min-w-0 px-half py-[2px] bg-secondary rounded border text-sm text-normal placeholder:text-low focus:outline-none focus:ring-1 focus:ring-brand"
        />
        <button
          type="button"
          onClick={() => void handleCreate()}
          disabled={isCreating}
          className="shrink-0 flex items-center gap-[2px] px-half py-[2px] bg-secondary rounded border text-sm text-normal hover:text-high disabled:opacity-50"
        >
          <PlusIcon className="h-3 w-3" />
          {t('browser.sessions.newSession')}
        </button>
      </div>

      {error && (
        <div className="px-half py-[2px] text-sm text-error truncate">
          {error}
        </div>
      )}

      {/* Session list */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {sessions.length === 0 ? (
          <div className="p-half text-sm text-low">
            {t('browser.sessions.empty')}
          </div>
        ) : (
          sessions.map(({ session, live }) => (
            <div
              key={session.id}
              role="button"
              tabIndex={0}
              onClick={() => setSelectedSessionId(session.id)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  setSelectedSessionId(session.id);
                }
              }}
              className={cn(
                'flex items-center gap-half px-half py-half cursor-pointer border-b hover:bg-secondary',
                selectedSessionId === session.id && 'bg-secondary'
              )}
            >
              <span
                className={cn(
                  'shrink-0 h-2 w-2 rounded-full',
                  browserStatusDotClass(live?.status ?? session.status)
                )}
                title={live?.status ?? session.status}
              />
              <div className="flex-1 min-w-0 flex flex-col">
                <span className="truncate text-sm text-normal">
                  {session.profile ?? t('browser.sessions.defaultProfile')}
                  {' · '}
                  {new Date(session.created_at).toLocaleString()}
                </span>
                <span className="truncate text-sm text-low">
                  {controllerSummary(live?.control.controller)}
                  {session.expires_at &&
                    ` · ${t('browser.sessions.expires')} ${new Date(
                      session.expires_at
                    ).toLocaleString()}`}
                </span>
              </div>
              <button
                type="button"
                title={t('browser.sessions.close')}
                onClick={(e) => {
                  e.stopPropagation();
                  void handleClose(session.id);
                }}
                className="shrink-0 flex items-center justify-center rounded text-low hover:text-high"
              >
                <XIcon className="h-3 w-3" />
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
