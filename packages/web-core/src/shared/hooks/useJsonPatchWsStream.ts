import { useCallback, useMemo, useSyncExternalStore } from 'react';
import type { Operation } from 'rfc6902';
import type { LocalApiWebSocketOptions } from '@/shared/lib/localApiTransport';
import { useHostId } from '@/shared/providers/HostIdProvider';
import {
  acquireSharedJsonPatchStream,
  idleJsonPatchStreamSnapshot,
  sharedJsonPatchStreamKey,
} from '@/shared/lib/sharedJsonPatchStream';

interface UseJsonPatchStreamOptions<T> {
  /**
   * Called once when the stream starts to inject initial data
   */
  injectInitialEntry?: (data: T) => void;
  /**
   * Filter/deduplicate patches before applying them
   */
  deduplicatePatches?: (patches: Operation[]) => Operation[];
  /**
   * Host scope for the socket. Defaults to the current route's host.
   */
  socketOptions?: LocalApiWebSocketOptions;
}

interface UseJsonPatchStreamResult<T> {
  data: T | undefined;
  isConnected: boolean;
  isInitialized: boolean;
  error: string | null;
}

const noopSubscribe = () => () => {};

/**
 * Generic hook for consuming WebSocket streams that send JSON messages with
 * patches.
 *
 * Every consumer of the same endpoint on the same host shares one socket
 * (see `sharedJsonPatchStream.ts`): each websocket is a separate handshake
 * through the edge proxy, and Safari opens them one at a time.
 */
export const useJsonPatchWsStream = <T extends object>(
  endpoint: string | undefined,
  enabled: boolean,
  initialData: () => T,
  options?: UseJsonPatchStreamOptions<T>
): UseJsonPatchStreamResult<T> => {
  const injectInitialEntry = options?.injectInitialEntry;
  const deduplicatePatches = options?.deduplicatePatches;
  const socketOptions = options?.socketOptions;
  // A different host is a different stream even when the path is identical.
  // The route context, not the module-level host id: that one is only
  // updated in a layout effect, after this render has already picked a key.
  const currentHostId = useHostId();
  const streamKey =
    enabled && endpoint
      ? sharedJsonPatchStreamKey(endpoint, socketOptions, currentHostId)
      : undefined;

  // Shared streams come from a registry keyed by streamKey, so recomputing
  // this for a new options object returns the same stream.
  const stream = useMemo(
    () =>
      streamKey && endpoint
        ? acquireSharedJsonPatchStream<T>(
            endpoint,
            {
              initialData,
              injectInitialEntry,
              deduplicatePatches,
              socketOptions,
            },
            currentHostId
          )
        : null,
    [
      streamKey,
      endpoint,
      currentHostId,
      initialData,
      injectInitialEntry,
      deduplicatePatches,
      socketOptions,
    ]
  );

  const subscribe = stream?.subscribe ?? noopSubscribe;
  const getSnapshot = useCallback(
    () => (stream ? stream.getSnapshot() : idleJsonPatchStreamSnapshot<T>()),
    [stream]
  );
  const snapshot = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  return {
    data: snapshot.data,
    isConnected: snapshot.isConnected,
    isInitialized: snapshot.isInitialized,
    error: snapshot.error,
  };
};
