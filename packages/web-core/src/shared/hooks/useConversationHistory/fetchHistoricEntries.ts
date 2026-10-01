import type {
  ExecutionProcess,
  ExecutionProcessLogSnapshot,
  PatchType,
} from 'shared/types';
import {
  makeLocalApiRequest,
  type LocalApiRequestOptions,
} from '@/shared/lib/localApiTransport';

export type HistoryFetchFailure =
  | 'timeout'
  | 'aborted'
  | 'network'
  | 'status'
  | 'api'
  | 'parse';

/** Why a completed turn's history request failed. Never means completion. */
export class HistoryFetchError extends Error {
  constructor(
    message: string,
    readonly reason: HistoryFetchFailure,
    readonly status?: number
  ) {
    super(message);
    this.name = 'HistoryFetchError';
  }
}

type LocalApiRequest = (
  pathOrUrl: string,
  init?: LocalApiRequestOptions
) => Promise<Response>;

export interface FetchLogSnapshotOptions {
  /** Total time allowed for the request, response and body. */
  deadlineMs: number;
  /** Caller cancellation, e.g. a scope change. */
  signal?: AbortSignal;
  /** Defaults to `makeLocalApiRequest` (host scoping, relay transport). */
  request?: LocalApiRequest;
}

type ProcessIdentity = Pick<ExecutionProcess, 'id' | 'executor_action'>;

/** Scripts keep raw output; agent and review turns are normalized. */
export function historicLogSnapshotPath(process: ProcessIdentity): string {
  const kind =
    process.executor_action.typ.type === 'ScriptRequest'
      ? 'raw-logs'
      : 'normalized-logs';
  return `/api/execution-processes/${process.id}/${kind}`;
}

function isLogSnapshot(value: unknown): value is ExecutionProcessLogSnapshot {
  if (!value || typeof value !== 'object') return false;
  const candidate = value as Partial<ExecutionProcessLogSnapshot>;
  return (
    Array.isArray(candidate.entries) && typeof candidate.complete === 'boolean'
  );
}

/**
 * Fetch one completed turn's entries over plain HTTP. A request reuses the
 * page's existing HTTP/2 or HTTP/3 connection, while every websocket pays its
 * own handshake through the edge proxy and Safari opens them one at a time.
 *
 * Settles exactly once. A deadline, a caller abort, a transport error, an
 * error status, `success: false` or a body that is not a snapshot all reject.
 * None of them is ever treated as an empty, finished turn.
 */
export function fetchProcessLogSnapshot(
  process: ProcessIdentity,
  { deadlineMs, signal, request = makeLocalApiRequest }: FetchLogSnapshotOptions
): Promise<ExecutionProcessLogSnapshot> {
  const path = historicLogSnapshotPath(process);

  return new Promise((resolve, reject) => {
    const controller = new AbortController();
    let settled = false;

    const finish = (settle: () => void) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      signal?.removeEventListener('abort', onCallerAbort);
      settle();
    };
    const fail = (error: HistoryFetchError) =>
      finish(() => {
        controller.abort();
        reject(error);
      });
    const onCallerAbort = () =>
      fail(
        new HistoryFetchError(`history request aborted: ${path}`, 'aborted')
      );

    const timer = setTimeout(
      () =>
        fail(
          new HistoryFetchError(
            `history request timed out after ${deadlineMs}ms: ${path}`,
            'timeout'
          )
        ),
      deadlineMs
    );

    if (signal?.aborted) {
      onCallerAbort();
      return;
    }
    signal?.addEventListener('abort', onCallerAbort);

    void (async () => {
      let response: Response;
      try {
        response = await request(path, { signal: controller.signal });
      } catch (error) {
        fail(
          new HistoryFetchError(
            `history request failed: ${path}: ${String(error)}`,
            'network'
          )
        );
        return;
      }

      if (!response.ok) {
        fail(
          new HistoryFetchError(
            `history request failed with status ${response.status}: ${path}`,
            'status',
            response.status
          )
        );
        return;
      }

      let body: unknown;
      try {
        body = await response.json();
      } catch {
        fail(
          new HistoryFetchError(
            `history response was not JSON: ${path}`,
            'parse'
          )
        );
        return;
      }

      const envelope = body as { success?: unknown; data?: unknown } | null;
      if (
        !envelope ||
        envelope.success !== true ||
        !isLogSnapshot(envelope.data)
      ) {
        fail(
          new HistoryFetchError(
            `history response was not a log snapshot: ${path}`,
            'api'
          )
        );
        return;
      }

      const snapshot = envelope.data;
      finish(() => resolve(snapshot));
    })();
  });
}

export interface HistoricEntriesResult {
  entries: PatchType[];
  /** Settled: safe to cache and to show as the whole turn. */
  complete: boolean;
}

export interface HistoricEntriesDeps {
  fetchSnapshot: (
    process: ExecutionProcess
  ) => Promise<ExecutionProcessLogSnapshot>;
  /**
   * The websocket replay, awaited until `finished`. Only used when the server
   * says its snapshot is not settled yet.
   */
  streamFallback: (process: ExecutionProcess) => Promise<PatchType[]>;
}

/**
 * Load a turn the process snapshot reported as not running.
 *
 * The HTTP snapshot is the normal path. When the server reports it as not
 * settled, the process finished between the snapshot and this fetch and its
 * log store is still live, or its row still says running. The entries are then
 * a prefix, so they are not returned. The websocket replay follows the store
 * until it drops and returns the complete turn instead. Any failure rejects,
 * so `loadProcessesInOrder` skips and counts the turn, and "load earlier"
 * retries it.
 */
export async function loadHistoricProcessEntries(
  process: ExecutionProcess,
  { fetchSnapshot, streamFallback }: HistoricEntriesDeps
): Promise<HistoricEntriesResult> {
  const snapshot = await fetchSnapshot(process);
  if (snapshot.complete) {
    return { entries: snapshot.entries, complete: true };
  }
  const entries = await streamFallback(process);
  return { entries, complete: true };
}
