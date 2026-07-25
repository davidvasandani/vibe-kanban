import { useCallback, useEffect, useRef, useState } from 'react';
import type {
  BrowserAction,
  BrowserActionResult,
  BrowserSessionError,
  BrowserSessionLiveState,
  BrowserTransferTargetRequest,
  BrowserWsClientMessage,
  BrowserWsServerMessage,
} from 'shared/types';
import { openLocalApiWebSocket } from '@/shared/lib/localApiTransport';
import {
  browserSessionWsReduce,
  initialBrowserWsReducerState,
  type BrowserFrameMeta,
  type BrowserWsEffect,
  type BrowserWsReducerState,
} from './browserSessionWsReducer';

interface PendingCommand {
  resolve: (result: BrowserActionResult | null) => void;
  reject: (error: unknown) => void;
}

export interface UseBrowserSessionWsResult {
  liveState: BrowserSessionLiveState | null;
  frameUrl: string | null;
  frameSize: { width: number; height: number } | null;
  connectionId: string | null;
  connected: boolean;
  lastError: BrowserSessionError | null;
  clearLastError: () => void;
  sendInput: (action: BrowserAction) => Promise<BrowserActionResult | null>;
  acquire: (opts?: { takeFromAgent?: boolean; force?: boolean }) => void;
  release: () => void;
  transfer: (target: BrowserTransferTargetRequest) => void;
}

/**
 * Manages the live-view WebSocket for a browser session.
 *
 * - JSON server messages update the live state via a pure reducer.
 * - A 'frame' JSON message is followed by one binary message carrying that
 *   frame's JPEG bytes; frames are exposed as an object URL.
 * - Human control acquired over this socket is bound to its connection_id and
 *   is auto-released by the server on disconnect.
 */
export function useBrowserSessionWs(
  sessionId: string | null
): UseBrowserSessionWsResult {
  const [liveState, setLiveState] = useState<BrowserSessionLiveState | null>(
    null
  );
  const [frameUrl, setFrameUrl] = useState<string | null>(null);
  const [frameSize, setFrameSize] = useState<BrowserFrameMeta | null>(null);
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [lastError, setLastError] = useState<BrowserSessionError | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const reducerStateRef = useRef<BrowserWsReducerState>(
    initialBrowserWsReducerState
  );
  const pendingCommandsRef = useRef<Map<string, PendingCommand>>(new Map());
  const frameUrlRef = useRef<string | null>(null);
  const retryTimerRef = useRef<number | null>(null);
  const retryAttemptsRef = useRef(0);
  const [retryNonce, setRetryNonce] = useState(0);

  const applyEffects = useCallback((effects: BrowserWsEffect[]) => {
    for (const effect of effects) {
      if (effect.kind === 'frame') {
        const url = URL.createObjectURL(
          new Blob([effect.data], { type: 'image/jpeg' })
        );
        if (frameUrlRef.current) {
          URL.revokeObjectURL(frameUrlRef.current);
        }
        frameUrlRef.current = url;
        setFrameUrl(url);
        setFrameSize({
          seq: effect.meta.seq,
          width: effect.meta.width,
          height: effect.meta.height,
        });
      } else if (effect.kind === 'command_result') {
        if (!effect.commandId) continue;
        const pending = pendingCommandsRef.current.get(effect.commandId);
        if (!pending) continue;
        pendingCommandsRef.current.delete(effect.commandId);
        if (effect.ok) {
          pending.resolve(effect.result);
        } else {
          pending.reject(effect.error ?? new Error('Browser command failed'));
        }
      }
    }
  }, []);

  useEffect(() => {
    if (!sessionId) return;

    let cancelled = false;

    const scheduleReconnect = () => {
      if (retryTimerRef.current !== null) return;
      const delay = Math.min(8000, 1000 * 2 ** retryAttemptsRef.current);
      retryTimerRef.current = window.setTimeout(() => {
        retryTimerRef.current = null;
        setRetryNonce((n) => n + 1);
      }, delay);
    };

    const rejectPending = (reason: string) => {
      for (const pending of pendingCommandsRef.current.values()) {
        pending.reject(new Error(reason));
      }
      pendingCommandsRef.current.clear();
    };

    void (async () => {
      try {
        const ws = await openLocalApiWebSocket(
          `/api/browser-sessions/${sessionId}/ws`
        );
        if (cancelled) {
          ws.close();
          return;
        }
        ws.binaryType = 'arraybuffer';

        ws.onopen = () => {
          retryAttemptsRef.current = 0;
          setConnected(true);
        };

        ws.onmessage = (event) => {
          try {
            const result =
              typeof event.data === 'string'
                ? browserSessionWsReduce(reducerStateRef.current, {
                    kind: 'json',
                    message: JSON.parse(event.data) as BrowserWsServerMessage,
                  })
                : browserSessionWsReduce(reducerStateRef.current, {
                    kind: 'binary',
                    data: event.data as ArrayBuffer,
                  });

            reducerStateRef.current = result.state;
            setLiveState(result.state.liveState);
            setConnectionId(result.state.connectionId);
            setLastError(result.state.lastError);
            applyEffects(result.effects);
          } catch (err) {
            console.error('[useBrowserSessionWs] message error:', err);
          }
        };

        ws.onclose = () => {
          wsRef.current = null;
          setConnected(false);
          // Control acquired over this socket is bound to its connection_id
          // and released on disconnect.
          setConnectionId(null);
          reducerStateRef.current = {
            ...reducerStateRef.current,
            connectionId: null,
            pendingFrameMeta: null,
          };
          rejectPending('Browser session connection closed');
          if (!cancelled) {
            retryAttemptsRef.current += 1;
            scheduleReconnect();
          }
        };

        wsRef.current = ws;
      } catch (err) {
        if (cancelled) return;
        console.error('[useBrowserSessionWs] failed to connect:', err);
        retryAttemptsRef.current += 1;
        scheduleReconnect();
      }
    })();

    return () => {
      cancelled = true;
      if (retryTimerRef.current !== null) {
        window.clearTimeout(retryTimerRef.current);
        retryTimerRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.onopen = null;
        wsRef.current.onmessage = null;
        wsRef.current.onclose = null;
        wsRef.current.close();
        wsRef.current = null;
      }
      rejectPending('Browser session connection closed');
      reducerStateRef.current = initialBrowserWsReducerState;
      if (frameUrlRef.current) {
        URL.revokeObjectURL(frameUrlRef.current);
        frameUrlRef.current = null;
      }
      setLiveState(null);
      setFrameUrl(null);
      setFrameSize(null);
      setConnectionId(null);
      setConnected(false);
      setLastError(null);
    };
  }, [sessionId, retryNonce, applyEffects]);

  const sendMessage = useCallback((message: BrowserWsClientMessage) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      return false;
    }
    ws.send(JSON.stringify(message));
    return true;
  }, []);

  const sendInput = useCallback(
    (action: BrowserAction): Promise<BrowserActionResult | null> => {
      const commandId = crypto.randomUUID();
      const expectedGeneration =
        reducerStateRef.current.liveState?.control.generation ?? null;

      return new Promise<BrowserActionResult | null>((resolve, reject) => {
        pendingCommandsRef.current.set(commandId, { resolve, reject });
        const sent = sendMessage({
          type: 'input',
          command_id: commandId,
          expected_generation: expectedGeneration,
          action,
        });
        if (!sent) {
          pendingCommandsRef.current.delete(commandId);
          reject(new Error('Browser session not connected'));
        }
      });
    },
    [sendMessage]
  );

  const acquire = useCallback(
    (opts?: { takeFromAgent?: boolean; force?: boolean }) => {
      sendMessage({
        type: 'acquire',
        take_from_agent: opts?.takeFromAgent ?? false,
        force: opts?.force ?? false,
        expected_generation:
          reducerStateRef.current.liveState?.control.generation ?? null,
      });
    },
    [sendMessage]
  );

  const release = useCallback(() => {
    sendMessage({
      type: 'release',
      expected_generation:
        reducerStateRef.current.liveState?.control.generation ?? null,
    });
  }, [sendMessage]);

  const transfer = useCallback(
    (target: BrowserTransferTargetRequest) => {
      const generation = reducerStateRef.current.liveState?.control.generation;
      if (generation === undefined) return;
      sendMessage({
        type: 'transfer',
        expected_generation: generation,
        target,
      });
    },
    [sendMessage]
  );

  const clearLastError = useCallback(() => {
    reducerStateRef.current = { ...reducerStateRef.current, lastError: null };
    setLastError(null);
  }, []);

  return {
    liveState,
    frameUrl,
    frameSize,
    connectionId,
    connected,
    lastError,
    clearLastError,
    sendInput,
    acquire,
    release,
    transfer,
  };
}
