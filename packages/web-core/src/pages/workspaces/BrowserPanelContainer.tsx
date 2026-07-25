import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { XIcon } from '@phosphor-icons/react';
import type {
  BrowserSessionDbStatus,
  BrowserSessionStatus,
  MouseButton,
} from 'shared/types';
import { browserSessionsApi } from '@/shared/lib/api';
import { cn } from '@/shared/lib/utils';
import { useBrowserSessionWs } from '@/shared/hooks/useBrowserSessionWs';
import { useExecutionProcesses } from '@/shared/hooks/useExecutionProcesses';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import {
  useBrowserSessionStore,
  useSelectedBrowserSessionId,
} from '@/shared/stores/useBrowserSessionStore';

interface BrowserPanelContainerProps {
  workspaceId: string;
  className: string;
}

const STATUS_DOT_CLASSES: Record<string, string> = {
  starting: 'bg-brand',
  running: 'bg-success',
  closed: 'bg-low',
  failed: 'bg-error',
};

export function browserStatusDotClass(
  status: BrowserSessionStatus | BrowserSessionDbStatus | undefined
): string {
  return (status && STATUS_DOT_CLASSES[status]) || 'bg-low';
}

function toMouseButton(button: number): MouseButton {
  if (button === 1) return 'middle';
  if (button === 2) return 'right';
  return 'left';
}

function keyModifiers(e: React.KeyboardEvent): string[] | null {
  const modifiers: string[] = [];
  if (e.ctrlKey) modifiers.push('ctrl');
  if (e.altKey) modifiers.push('alt');
  if (e.shiftKey) modifiers.push('shift');
  if (e.metaKey) modifiers.push('meta');
  return modifiers.length > 0 ? modifiers : null;
}

export function BrowserPanelContainer({
  workspaceId,
  className,
}: BrowserPanelContainerProps) {
  const { t } = useTranslation('tasks');
  const [selectedSessionId, setSelectedSessionId] =
    useSelectedBrowserSessionId(workspaceId);
  const bumpListRefresh = useBrowserSessionStore((s) => s.bumpListRefresh);
  const [createError, setCreateError] = useState<string | null>(null);
  const [transferOpen, setTransferOpen] = useState(false);
  const [transferExecutionId, setTransferExecutionId] = useState('');

  const { selectedSession } = useWorkspaceContext();
  const { executionProcesses } = useExecutionProcesses(selectedSession?.id);

  const {
    liveState,
    frameUrl,
    frameSize,
    connectionId,
    connected,
    lastError,
    sendInput,
    acquire,
    release,
    transfer,
  } = useBrowserSessionWs(selectedSessionId);

  // Select an open session for this workspace, creating one if none exists.
  useEffect(() => {
    if (selectedSessionId) return;
    let cancelled = false;

    void (async () => {
      try {
        const sessions = await browserSessionsApi.list(workspaceId);
        if (cancelled) return;
        const open = sessions.find(
          (s) => s.session.status !== 'closed' && s.session.status !== 'failed'
        );
        if (open) {
          setSelectedSessionId(open.session.id);
          return;
        }
        const created = await browserSessionsApi.create({
          workspace_id: workspaceId,
          profile: null,
        });
        if (cancelled) return;
        setCreateError(null);
        setSelectedSessionId(created.session.id);
        bumpListRefresh();
      } catch (err) {
        if (cancelled) return;
        setCreateError(
          err instanceof Error ? err.message : t('browser.errors.createFailed')
        );
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [
    workspaceId,
    selectedSessionId,
    setSelectedSessionId,
    bumpListRefresh,
    t,
  ]);

  // Refresh the session list when live state changes over the WS.
  const controlGeneration = liveState?.control.generation;
  const liveStatus = liveState?.status;
  useEffect(() => {
    if (controlGeneration === undefined) return;
    bumpListRefresh();
  }, [controlGeneration, liveStatus, bumpListRefresh]);

  const controller = liveState?.control.controller;
  const weControl =
    controller?.type === 'human' &&
    connectionId !== null &&
    controller.connection_id === connectionId;
  const canTakeControl =
    controller?.type === 'agent' || controller?.type === 'none';

  const handleTakeControl = useCallback(() => {
    acquire({ takeFromAgent: true });
  }, [acquire]);

  const handleRelease = useCallback(() => {
    setTransferOpen(false);
    release();
  }, [release]);

  const handleTransferToExecution = useCallback(
    (executionId: string) => {
      const trimmed = executionId.trim();
      if (!trimmed) return;
      transfer({ type: 'agent', execution_id: trimmed });
      setTransferOpen(false);
      setTransferExecutionId('');
    },
    [transfer]
  );

  // ── Input capture (only while we hold control) ─────────────────────────
  const imgRef = useRef<HTMLImageElement>(null);
  const lastMouseMoveAtRef = useRef(0);

  const toFrameCoords = useCallback(
    (clientX: number, clientY: number): { x: number; y: number } | null => {
      const img = imgRef.current;
      if (!img || !frameSize) return null;
      const rect = img.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) return null;
      // object-contain: the frame is letterboxed inside the rendered box.
      const scale = Math.min(
        rect.width / frameSize.width,
        rect.height / frameSize.height
      );
      if (scale <= 0) return null;
      const offsetX = (rect.width - frameSize.width * scale) / 2;
      const offsetY = (rect.height - frameSize.height * scale) / 2;
      const x = (clientX - rect.left - offsetX) / scale;
      const y = (clientY - rect.top - offsetY) / scale;
      if (x < 0 || y < 0 || x > frameSize.width || y > frameSize.height) {
        return null;
      }
      return { x: Math.round(x), y: Math.round(y) };
    },
    [frameSize]
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (!weControl) return;
      const coords = toFrameCoords(e.clientX, e.clientY);
      if (!coords) return;
      e.preventDefault();
      (e.currentTarget as HTMLElement).focus();
      void sendInput({
        type: 'click',
        x: coords.x,
        y: coords.y,
        button: toMouseButton(e.button),
      }).catch(() => {});
    },
    [weControl, toFrameCoords, sendInput]
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!weControl) return;
      const now = Date.now();
      // Coalesce mouse moves to at most one input per 100ms.
      if (now - lastMouseMoveAtRef.current < 100) return;
      const coords = toFrameCoords(e.clientX, e.clientY);
      if (!coords) return;
      lastMouseMoveAtRef.current = now;
      void sendInput({
        type: 'mouse_move',
        x: coords.x,
        y: coords.y,
      }).catch(() => {});
    },
    [weControl, toFrameCoords, sendInput]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!weControl) return;
      e.preventDefault();
      void sendInput({
        type: 'key',
        key: e.key,
        modifiers: keyModifiers(e),
      }).catch(() => {});
    },
    [weControl, sendInput]
  );

  // ── Derived toolbar state ──────────────────────────────────────────────
  const controllerText = (() => {
    if (weControl) return t('browser.controller.you');
    if (controller?.type === 'agent') {
      return `${t('browser.controller.agent')} · ${controller.execution_id.slice(0, 8)}`;
    }
    if (controller?.type === 'human') return t('browser.controller.human');
    return t('browser.controller.none');
  })();

  const bannerText = (() => {
    if (createError) return createError;
    if (liveState?.status === 'closed') {
      return t('browser.errors.sessionClosed');
    }
    if (lastError?.code === 'BROWSER_UNAVAILABLE') {
      return `BROWSER_UNAVAILABLE: ${lastError.message}`;
    }
    if (lastError) return lastError.code;
    return null;
  })();

  const buttonClass =
    'shrink-0 px-half py-[2px] bg-secondary rounded border text-sm text-normal hover:text-high';

  return (
    <div className={cn('flex flex-col h-full min-h-0 bg-primary', className)}>
      {/* Toolbar */}
      <div className="flex items-center gap-half px-base py-half border-b bg-secondary">
        <span
          className={cn(
            'shrink-0 h-2 w-2 rounded-full',
            browserStatusDotClass(liveState?.status)
          )}
          title={liveState?.status ?? 'unknown'}
        />
        <span className="flex-1 min-w-0 truncate text-sm text-low font-ibm-plex-mono">
          {liveState?.current_url ?? ''}
        </span>
        <span className="shrink-0 text-sm text-normal">{controllerText}</span>
        {canTakeControl && (
          <button
            type="button"
            className={buttonClass}
            onClick={handleTakeControl}
            disabled={!connected}
          >
            {t('browser.toolbar.takeControl')}
          </button>
        )}
        {weControl && (
          <>
            <button
              type="button"
              className={buttonClass}
              onClick={handleRelease}
            >
              {t('browser.toolbar.release')}
            </button>
            <div className="relative shrink-0">
              <button
                type="button"
                className={buttonClass}
                onClick={() => setTransferOpen((open) => !open)}
              >
                {t('browser.toolbar.returnToAgent')}
              </button>
              {transferOpen && (
                <div className="absolute right-0 top-full mt-half z-10 w-64 p-half bg-panel border rounded shadow-md flex flex-col gap-half">
                  {executionProcesses.length > 0 ? (
                    [...executionProcesses]
                      .reverse()
                      .slice(0, 5)
                      .map((process) => (
                        <button
                          key={process.id}
                          type="button"
                          className="text-left px-half py-[2px] rounded text-sm text-normal hover:bg-secondary truncate"
                          onClick={() => handleTransferToExecution(process.id)}
                        >
                          {process.id.slice(0, 8)} · {process.run_reason} ·{' '}
                          {process.status}
                        </button>
                      ))
                  ) : (
                    <span className="px-half text-sm text-low">
                      {t('browser.toolbar.noExecutions')}
                    </span>
                  )}
                  <div className="flex items-center gap-half border-t pt-half">
                    <input
                      value={transferExecutionId}
                      onChange={(e) => setTransferExecutionId(e.target.value)}
                      placeholder={t('browser.toolbar.executionPlaceholder')}
                      className="flex-1 min-w-0 px-half py-[2px] bg-secondary rounded border text-sm text-normal placeholder:text-low focus:outline-none focus:ring-1 focus:ring-brand"
                    />
                    <button
                      type="button"
                      className={buttonClass}
                      onClick={() =>
                        handleTransferToExecution(transferExecutionId)
                      }
                    >
                      {t('browser.toolbar.return')}
                    </button>
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {/* Error banner */}
      {bannerText && (
        <div className="flex items-center gap-half px-base py-half bg-error/10 text-error text-sm border-b">
          <XIcon className="shrink-0 h-3 w-3" />
          <span className="truncate">{bannerText}</span>
        </div>
      )}

      {/* Live view */}
      <div
        className="flex-1 min-h-0 flex items-center justify-center bg-black outline-none"
        tabIndex={0}
        onPointerUp={handlePointerUp}
        onPointerMove={handlePointerMove}
        onKeyDown={handleKeyDown}
        style={{ cursor: weControl ? 'crosshair' : 'default' }}
      >
        {frameUrl ? (
          <img
            ref={imgRef}
            src={frameUrl}
            alt={liveState?.page_title ?? 'Browser session'}
            className="w-full h-full object-contain select-none"
            draggable={false}
          />
        ) : (
          <span className="text-sm text-low">
            {connected
              ? t('browser.view.waitingForFrames')
              : t('browser.view.connecting')}
          </span>
        )}
      </div>
    </div>
  );
}
