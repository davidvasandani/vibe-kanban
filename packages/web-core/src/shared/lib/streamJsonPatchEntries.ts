// streamJsonPatchEntries.ts - WebSocket JSON patch streaming utility
import { produce } from 'immer';
import type { Operation } from 'rfc6902';
import { applyUpsertPatch } from '@/shared/lib/jsonPatch';
import { openLocalApiWebSocket } from '@/shared/lib/localApiTransport';

type PatchContainer<E = unknown> = { entries: E[] };

export interface StreamOptions<E = unknown> {
  initial?: PatchContainer<E>;
  /** called after each successful patch application */
  onEntries?: (entries: E[]) => void;
  onConnect?: () => void;
  /**
   * called once when the stream ends without "finished": transport error,
   * close, unparseable message, or idle timeout
   */
  onError?: (err: unknown) => void;
  /** called once when a "finished" event is received */
  onFinished?: (entries: E[]) => void;
  /**
   * Fail the stream if no message arrives for this long. Reset on every
   * message. Leave unset for live streams, which may be legitimately silent.
   */
  idleTimeoutMs?: number;
}

interface StreamController<E = unknown> {
  /** Current entries array (immutable snapshot) */
  getEntries(): E[];
  /** Full { entries } snapshot */
  getSnapshot(): PatchContainer<E>;
  /** Best-effort connection state */
  isConnected(): boolean;
  /** Subscribe to updates; returns an unsubscribe function */
  onChange(cb: (entries: E[]) => void): () => void;
  /** Close the stream */
  close(): void;
}

/**
 * Connect to a WebSocket endpoint that emits JSON messages containing:
 *   {"JsonPatch": [{"op": "add", "path": "/entries/0", "value": {...}}, ...]}
 *   {"Finished": ""}
 *
 * Maintains an in-memory { entries: [] } snapshot and returns a controller.
 *
 * Exactly one of onFinished / onError fires per stream unless the caller
 * closes it first. A close is never treated as completion: the server closes
 * cleanly after a log read fails, and proxies and mobile OSes drop sockets,
 * so a waiter keyed only on "finished" would otherwise hang forever.
 *
 * Messages are batched per animation frame and applied using immer for
 * structural sharing, avoiding a full deep clone on every message.
 */
export function streamJsonPatchEntries<E = unknown>(
  url: string,
  opts: StreamOptions<E> = {}
): StreamController<E> {
  let connected = false;
  let closed = false;
  let settled = false;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;
  let ws: WebSocket | null = null;
  let snapshot: PatchContainer<E> = structuredClone(
    opts.initial ?? ({ entries: [] } as PatchContainer<E>)
  );

  const subscribers = new Set<(entries: E[]) => void>();
  if (opts.onEntries) subscribers.add(opts.onEntries);

  // --- rAF batching state ---
  let pendingOps: Operation[] = [];
  let rafId: number | null = null;

  const notify = () => {
    for (const cb of subscribers) {
      try {
        cb(snapshot.entries);
      } catch {
        /* swallow subscriber errors */
      }
    }
  };

  const flush = () => {
    rafId = null;
    if (pendingOps.length === 0) return;

    const ops = dedupeOps(pendingOps);
    pendingOps = [];

    snapshot = produce(snapshot, (draft) => {
      applyUpsertPatch(draft, ops);
    });
    notify();
  };

  const clearIdleTimer = () => {
    if (idleTimer !== null) {
      clearTimeout(idleTimer);
      idleTimer = null;
    }
  };

  const fail = (err: unknown) => {
    if (settled || closed) return;
    settled = true;
    clearIdleTimer();
    ws?.close();
    opts.onError?.(err);
  };

  const armIdleTimer = () => {
    if (opts.idleTimeoutMs === undefined || settled || closed) return;
    clearIdleTimer();
    idleTimer = setTimeout(() => {
      idleTimer = null;
      fail(
        new Error(
          `stream idle for ${opts.idleTimeoutMs}ms before finished: ${url}`
        )
      );
    }, opts.idleTimeoutMs);
  };

  const handleMessage = (event: MessageEvent) => {
    if (settled || closed) return;
    armIdleTimer();
    try {
      const msg = JSON.parse(event.data);

      // Handle JsonPatch messages — accumulate ops for next rAF flush
      if (msg.JsonPatch) {
        const raw = msg.JsonPatch as Operation[];
        pendingOps.push(...raw);
        if (rafId === null) {
          rafId = requestAnimationFrame(flush);
        }
      }

      // Handle Finished messages — flush synchronously before closing
      if (msg.finished !== undefined) {
        if (rafId !== null) {
          cancelAnimationFrame(rafId);
        }
        flush();
        settled = true;
        clearIdleTimer();
        opts.onFinished?.(snapshot.entries);
        ws?.close();
      }
    } catch (err) {
      fail(err);
    }
  };

  armIdleTimer();

  void (async () => {
    try {
      const opened = await openLocalApiWebSocket(url);

      // An idle timeout can settle the stream while the socket is opening.
      if (closed || settled) {
        opened.close();
        return;
      }

      ws = opened;
      ws.addEventListener('open', () => {
        connected = true;
        opts.onConnect?.();
      });

      ws.addEventListener('message', handleMessage);

      ws.addEventListener('error', (err) => {
        connected = false;
        fail(err);
      });

      ws.addEventListener('close', (event) => {
        connected = false;
        if (rafId !== null) {
          cancelAnimationFrame(rafId);
          rafId = null;
        }
        fail(
          new Error(
            `stream closed before finished (code ${event.code}): ${url}`
          )
        );
      });
    } catch (error) {
      fail(error);
    }
  })();

  return {
    getEntries(): E[] {
      return snapshot.entries;
    },
    getSnapshot(): PatchContainer<E> {
      return snapshot;
    },
    isConnected(): boolean {
      return connected;
    },
    onChange(cb: (entries: E[]) => void): () => void {
      subscribers.add(cb);
      // push current state immediately
      cb(snapshot.entries);
      return () => subscribers.delete(cb);
    },
    close(): void {
      closed = true;
      clearIdleTimer();
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      ws?.close();
      subscribers.clear();
      connected = false;
    },
  };
}

/**
 * Dedupe multiple ops that touch the same path within a batch.
 * Last write for a path wins, while preserving the overall left-to-right
 * order of the *kept* final operations.
 *
 * Example:
 *   add /entries/4, replace /entries/4  -> keep only the final replace
 */
function dedupeOps(ops: Operation[]): Operation[] {
  const lastIndexByPath = new Map<string, number>();
  ops.forEach((op, i) => lastIndexByPath.set(op.path, i));

  // Keep only the last op for each path, in ascending order of their final index
  const keptIndices = [...lastIndexByPath.values()].sort((a, b) => a - b);
  return keptIndices.map((i) => ops[i]!);
}
