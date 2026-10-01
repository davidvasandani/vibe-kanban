import { produce } from 'immer';
import type { Operation } from 'rfc6902';
import { applyUpsertPatch } from '@/shared/lib/jsonPatch';
import {
  openLocalApiWebSocket,
  type LocalApiWebSocketOptions,
} from '@/shared/lib/localApiTransport';
import { getCurrentHostId } from '@/shared/providers/HostIdProvider';

type WsJsonPatchMsg = { JsonPatch: Operation[] };
type WsReadyMsg = { Ready: true };
type WsFinishedMsg = { finished: boolean };
type WsMsg = WsJsonPatchMsg | WsReadyMsg | WsFinishedMsg;

const MAX_RECONNECT_DELAY_MS = 8_000;

/**
 * How long a stream stays open after its last subscriber leaves. Components
 * remount, and sockets churn as a page settles (approvals and discovery were
 * seen closing and reopening within ~150 ms). A resubscribe inside this window
 * reuses the open socket and its snapshot instead of paying a new handshake.
 */
export const STREAM_LINGER_MS = 3_000;

export function getReconnectDelay(
  attempt: number,
  random: () => number = Math.random
): number {
  const exponential = Math.min(
    MAX_RECONNECT_DELAY_MS,
    1_000 * 2 ** Math.max(0, attempt)
  );
  // ±20% jitter prevents every stream and browser tab from reconnecting in a
  // synchronized burst when the replacement server becomes available.
  const jitter = 0.8 + Math.min(1, Math.max(0, random())) * 0.4;
  return Math.min(MAX_RECONNECT_DELAY_MS, Math.round(exponential * jitter));
}

export interface JsonPatchStreamSnapshot<T> {
  data: T | undefined;
  isConnected: boolean;
  isInitialized: boolean;
  error: string | null;
}

const IDLE_SNAPSHOT: JsonPatchStreamSnapshot<never> = Object.freeze({
  data: undefined,
  isConnected: false,
  isInitialized: false,
  error: null,
});

export function idleJsonPatchStreamSnapshot<T>(): JsonPatchStreamSnapshot<T> {
  return IDLE_SNAPSHOT;
}

export interface SharedJsonPatchStreamOptions<T> {
  initialData: () => T;
  injectInitialEntry?: (data: T) => void;
  deduplicatePatches?: (patches: Operation[]) => Operation[];
  socketOptions?: LocalApiWebSocketOptions;
}

/**
 * One JSON-patch websocket, shared by every subscriber of the same endpoint on
 * the same host. Each websocket is a new TCP+TLS handshake through the edge
 * proxy and iOS Safari opens them one at a time, so two components showing
 * the same stream must not pay for it twice.
 *
 * Semantics are those the per-component hook had: the snapshot is kept while
 * the same endpoint reconnects, only `Ready` resets backoff, `finished` is
 * terminal, a clean 1000 close does not reconnect, and the error surfaces
 * only after repeated failures with no authoritative snapshot.
 */
export class SharedJsonPatchStream<T extends object> {
  private snapshot: JsonPatchStreamSnapshot<T> = IDLE_SNAPSHOT;
  /**
   * The document patches apply to. It starts as `initialData()`, but is only
   * published as `data` once a patch has been applied: `initialData` is a
   * local placeholder, and consumers treat `data === undefined` as "nothing
   * received yet".
   */
  private working: T | undefined;
  private readonly listeners = new Set<() => void>();
  private ws: WebSocket | null = null;
  private opening = false;
  private generation = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private lingerTimer: ReturnType<typeof setTimeout> | null = null;
  private retryAttempts = 0;
  private finished = false;
  private active = false;

  constructor(
    readonly endpoint: string,
    private readonly options: SharedJsonPatchStreamOptions<T>
  ) {}

  readonly getSnapshot = (): JsonPatchStreamSnapshot<T> => this.snapshot;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    if (this.lingerTimer !== null) {
      clearTimeout(this.lingerTimer);
      this.lingerTimer = null;
    }
    if (!this.active) this.start();
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size === 0 && this.lingerTimer === null) {
        this.lingerTimer = setTimeout(() => {
          this.lingerTimer = null;
          if (this.listeners.size === 0) this.stop();
        }, STREAM_LINGER_MS);
      }
    };
  };

  /** Number of live subscribers (for tests and diagnostics). */
  get subscriberCount(): number {
    return this.listeners.size;
  }

  private update(patch: Partial<JsonPatchStreamSnapshot<T>>) {
    const next = { ...this.snapshot, ...patch };
    if (
      next.data === this.snapshot.data &&
      next.isConnected === this.snapshot.isConnected &&
      next.isInitialized === this.snapshot.isInitialized &&
      next.error === this.snapshot.error
    ) {
      return;
    }
    this.snapshot = next;
    for (const listener of [...this.listeners]) listener();
  }

  private start() {
    this.active = true;
    this.finished = false;
    this.retryAttempts = 0;
    const data = this.options.initialData();
    this.options.injectInitialEntry?.(data);
    this.working = data;
    this.update({
      data: undefined,
      isConnected: false,
      isInitialized: false,
      error: null,
    });
    this.connect();
  }

  private stop() {
    this.active = false;
    this.generation += 1;
    if (this.retryTimer !== null) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    const ws = this.ws;
    this.ws = null;
    this.opening = false;
    if (ws) {
      ws.onopen = null;
      ws.onmessage = null;
      ws.onerror = null;
      ws.onclose = null;
      ws.close();
    }
    this.finished = false;
    this.retryAttempts = 0;
    this.working = undefined;
    this.snapshot = IDLE_SNAPSHOT;
    for (const listener of [...this.listeners]) listener();
  }

  private scheduleReconnect() {
    if (this.retryTimer !== null || !this.active) return;
    const delay = getReconnectDelay(this.retryAttempts);
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null;
      if (this.active && !this.ws && !this.opening) this.connect();
    }, delay);
  }

  private recordFailure() {
    this.retryAttempts += 1;
    // initialData is only a local placeholder. A bounded connection error
    // depends on whether an authoritative Ready was ever received.
    if (!this.snapshot.isInitialized && this.retryAttempts > 6) {
      this.update({ error: 'Connection failed' });
    }
    this.scheduleReconnect();
  }

  private connect() {
    if (this.ws || this.opening) return;
    this.opening = true;
    this.finished = false;
    const generation = this.generation;

    void (async () => {
      let ws: WebSocket;
      try {
        const socketOptions = this.options.socketOptions;
        ws = socketOptions
          ? await openLocalApiWebSocket(this.endpoint, socketOptions)
          : await openLocalApiWebSocket(this.endpoint);
      } catch (error) {
        if (generation !== this.generation) return;
        this.opening = false;
        console.error('Failed to open WebSocket stream:', error);
        this.recordFailure();
        return;
      }

      if (generation !== this.generation) {
        ws.close();
        return;
      }
      this.opening = false;
      this.ws = ws;

      ws.onopen = () => {
        this.update({ isConnected: true });
        // Opening the transport does not prove this patch stream is
        // authoritative. Only Ready resets failure pressure.
        if (this.retryTimer !== null) {
          clearTimeout(this.retryTimer);
          this.retryTimer = null;
        }
      };

      ws.onmessage = (event) => {
        try {
          const msg: WsMsg = JSON.parse(event.data);

          if ('JsonPatch' in msg) {
            const patches = msg.JsonPatch;
            const filtered = this.options.deduplicatePatches
              ? this.options.deduplicatePatches(patches)
              : patches;
            const current = this.working;
            if (filtered.length && current) {
              // Immer for structural sharing: only modified parts get new
              // references.
              const next = produce(current, (draft) => {
                applyUpsertPatch(draft, filtered);
              });
              this.working = next;
              this.update({ data: next });
            }
          }

          if ('Ready' in msg) {
            this.retryAttempts = 0;
            this.update({ isInitialized: true, error: null });
          }

          // Treat finished as terminal: do NOT reconnect.
          if ('finished' in msg) {
            this.finished = true;
            ws.close(1000, 'finished');
            if (this.ws === ws) this.ws = null;
            this.update({ isConnected: false });
          }
        } catch (err) {
          console.error('Failed to process WebSocket message:', err);
          this.update({ error: 'Failed to process stream update' });
        }
      };

      ws.onerror = () => {
        // onclose always follows onerror and owns the retry decision.
        // Setting an error here would hide data that was already received.
      };

      ws.onclose = (evt) => {
        if (this.ws === ws) this.ws = null;
        this.update({ isConnected: false });
        if (
          !this.active ||
          this.finished ||
          (evt?.code === 1000 && evt?.wasClean)
        ) {
          return;
        }
        this.recordFailure();
      };
    })();
  }
}

const registry = new Map<string, SharedJsonPatchStream<object>>();
let privateStreamCounter = 0;

/** The identity two consumers must share to share a socket. */
export function sharedJsonPatchStreamKey(
  endpoint: string,
  socketOptions?: LocalApiWebSocketOptions,
  currentHostId: string | null = getCurrentHostId()
): string {
  const hostScope = socketOptions?.hostScope ?? 'current';
  // A stream for the route's current host must not be reused after the route
  // moves to another host, even while the old one is still lingering.
  const hostId =
    hostScope === 'current'
      ? (currentHostId ?? '')
      : (socketOptions?.hostId ?? '');
  return [endpoint, hostScope, hostId, socketOptions?.relayHostId ?? ''].join(
    '|'
  );
}

/**
 * The shared stream for `endpoint` on its resolved host, created on first use.
 * Nothing connects until someone subscribes. Streams whose consumers alter
 * the data (`injectInitialEntry`, `deduplicatePatches`) are kept private.
 */
export function acquireSharedJsonPatchStream<T extends object>(
  endpoint: string,
  options: SharedJsonPatchStreamOptions<T>,
  currentHostId: string | null = getCurrentHostId()
): SharedJsonPatchStream<T> {
  const isPrivate = Boolean(
    options.injectInitialEntry || options.deduplicatePatches
  );
  const key = isPrivate
    ? `private:${(privateStreamCounter += 1)}`
    : sharedJsonPatchStreamKey(endpoint, options.socketOptions, currentHostId);
  const existing = registry.get(key);
  if (existing) return existing as unknown as SharedJsonPatchStream<T>;
  const created = new SharedJsonPatchStream<T>(endpoint, options);
  if (!isPrivate) {
    registry.set(key, created as unknown as SharedJsonPatchStream<object>);
  }
  return created;
}

/** Close and forget every shared stream. Tests only. */
export function resetSharedJsonPatchStreamsForTests() {
  for (const stream of registry.values()) {
    (stream as unknown as { stop: () => void }).stop();
  }
  registry.clear();
}
