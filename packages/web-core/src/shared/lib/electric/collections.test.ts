import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Behavioural tests for the Electric hybrid sync in `collections.ts`.
 *
 * The hybrid sync tries ElectricSQL first, then locks the source to a REST
 * fallback if Electric never becomes ready within ELECTRIC_READY_TIMEOUT_MS
 * (3000ms). These tests assert the contract around that transition:
 *  - the timeout → fallback switch is silent (no user-facing sync error);
 *  - a successful fallback load reports recovery (so a stale error can clear);
 *  - a fallback that *fails* still surfaces a genuine error.
 */

const mocks = vi.hoisted(() => ({
  makeRequest: vi.fn(),
  getRemoteApiUrl: vi.fn(() => 'http://api.test'),
  getRemoteApiBasicAuth: vi.fn(() => null),
  registerShape: vi.fn(),
  getToken: vi.fn(async () => 'token'),
  triggerRefresh: vi.fn(async () => {}),
  // Electric's own sync: never marks the collection ready, forcing a timeout.
  electricSyncFn: vi.fn(() => undefined),
}));

vi.mock('@/shared/lib/remoteApi', () => ({
  makeRequest: mocks.makeRequest,
  getRemoteApiUrl: mocks.getRemoteApiUrl,
  getRemoteApiBasicAuth: mocks.getRemoteApiBasicAuth,
}));

vi.mock('@/shared/lib/auth/runtime', () => ({
  getAuthRuntime: () => ({
    registerShape: mocks.registerShape,
    getToken: mocks.getToken,
    triggerRefresh: mocks.triggerRefresh,
  }),
}));

vi.mock('@tanstack/electric-db-collection', () => ({
  electricCollectionOptions: (options: Record<string, unknown>) => ({
    ...options,
    sync: { sync: mocks.electricSyncFn },
  }),
}));

vi.mock('@tanstack/react-db', () => ({
  // Capture the fully-wrapped options so the test can drive the hybrid sync.
  createCollection: (options: Record<string, unknown>) => ({
    __options: options,
  }),
}));

import { createShapeCollection } from './collections';

const READY_TIMEOUT_MS = 3000;

type FakeSyncParams = {
  collection: {
    isReady: () => boolean;
    onFirstReady: (cb: () => void) => void;
  };
  begin: () => void;
  write: (message: { value: Record<string, unknown> }) => void;
  commit: () => void;
  markReady: () => void;
  truncate: () => void;
  __writes: Array<Record<string, unknown>>;
  __markReadyCount: number;
};

function makeFakeSyncParams(): FakeSyncParams {
  let ready = false;
  const params: FakeSyncParams = {
    collection: {
      isReady: () => ready,
      onFirstReady: () => {},
    },
    begin: () => {},
    write: (message) => {
      params.__writes.push(message.value);
    },
    commit: () => {},
    markReady: () => {
      ready = true;
      params.__markReadyCount += 1;
    },
    truncate: () => {
      params.__writes.length = 0;
    },
    __writes: [],
    __markReadyCount: 0,
  };
  return params;
}

let tableCounter = 0;
function uniqueShape() {
  tableCounter += 1;
  const table = `test_table_${tableCounter}`;
  return {
    table,
    url: `/shape/${table}`,
    fallbackUrl: `/api/${table}`,
  } as never;
}

/** Start the hybrid sync for a freshly-created collection. */
function startSync(collection: unknown, syncParams: FakeSyncParams) {
  const options = (collection as { __options: { sync: { sync: unknown } } })
    .__options;
  (options.sync.sync as (p: FakeSyncParams) => unknown)(syncParams);
}

describe('createShapeCollection hybrid sync', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    (globalThis as { document?: unknown }).document = {
      visibilityState: 'visible',
    };
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    delete (globalThis as { document?: unknown }).document;
  });

  it('switches to fallback silently on ready timeout and reports recovery', async () => {
    const shape = uniqueShape();
    const onError = vi.fn();
    const onRecovered = vi.fn();

    mocks.makeRequest.mockResolvedValue({
      ok: true,
      json: async () => ({
        [(shape as { table: string }).table]: [{ id: '1', name: 'row-1' }],
      }),
    });

    const collection = createShapeCollection(
      shape,
      { project_id: 'p1' },
      {
        onError,
        onRecovered,
      }
    );

    const syncParams = makeFakeSyncParams();
    startSync(collection, syncParams);

    // Electric is still not ready when the timeout elapses.
    await vi.advanceTimersByTimeAsync(READY_TIMEOUT_MS);

    // The timeout must NOT surface a user-facing sync error...
    expect(onError).not.toHaveBeenCalled();
    // ...it must switch to the REST fallback...
    expect(mocks.makeRequest).toHaveBeenCalledTimes(1);
    const requestedPath = mocks.makeRequest.mock.calls[0][0] as string;
    expect(requestedPath).toContain(
      `/api/${(shape as { table: string }).table}`
    );
    // ...apply the fetched rows and mark ready...
    expect(syncParams.__markReadyCount).toBeGreaterThan(0);
    expect(syncParams.__writes).toEqual([{ id: '1', name: 'row-1' }]);
    // ...and report that the source recovered so any stale error clears.
    expect(onRecovered).toHaveBeenCalledTimes(1);
  });

  it('surfaces an error when the fallback itself fails', async () => {
    const shape = uniqueShape();
    const onError = vi.fn();
    const onRecovered = vi.fn();

    mocks.makeRequest.mockResolvedValue({
      ok: false,
      json: async () => ({ message: 'fallback boom' }),
    });

    const collection = createShapeCollection(
      shape,
      { project_id: 'p2' },
      {
        onError,
        onRecovered,
      }
    );

    const syncParams = makeFakeSyncParams();
    startSync(collection, syncParams);

    await vi.advanceTimersByTimeAsync(READY_TIMEOUT_MS);

    // A genuinely-failing fallback is a real error the user should see.
    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0][0]).toEqual({ message: 'fallback boom' });
    // No recovery happened.
    expect(onRecovered).not.toHaveBeenCalled();
  });

  it('re-reports a same-message fallback error after a recovery', async () => {
    // Regression guard: recovery must also reset the error-report debounce, so
    // a fresh failure with the same message isn't suppressed as a "duplicate"
    // against stale state. Mutations trigger fallback refreshes back-to-back
    // (faster than the 30s poll), which is exactly when the debounce bites.
    const shape = uniqueShape();
    const table = (shape as { table: string }).table;
    const onError = vi.fn();
    const onRecovered = vi.fn();

    const failResp = { ok: false, json: async () => ({ message: 'boom' }) };
    const okResp = {
      ok: true,
      json: async () => ({ [table]: [{ id: '1' }] }),
    };

    // The fallback GET responses, consumed in order: initial poll fails, the
    // first mutation-triggered refresh recovers, the second fails again.
    const fallbackResponses = [failResp, okResp, failResp];

    mocks.makeRequest.mockImplementation(async (_path, init) => {
      const method = (init as RequestInit | undefined)?.method ?? 'GET';
      if (method !== 'GET') {
        // Mutation write (POST/PATCH/DELETE) — always succeeds.
        return { ok: true, json: async () => ({ txid: 1 }) };
      }
      return fallbackResponses.shift() ?? failResp;
    });

    const mutationDef = { name: 'thing', url: '/api/thing' } as never;
    const collection = createShapeCollection(
      shape,
      { project_id: 'p3' },
      { onError, onRecovered },
      mutationDef
    );
    const options = (
      collection as unknown as {
        __options: {
          onInsert: (p: {
            transaction: { mutations: Array<{ modified: unknown }> };
          }) => Promise<unknown>;
        };
      }
    ).__options;

    const syncParams = makeFakeSyncParams();
    startSync(collection, syncParams);

    // Initial fallback poll fails → first error reported.
    await vi.advanceTimersByTimeAsync(READY_TIMEOUT_MS);
    expect(onError).toHaveBeenCalledTimes(1);

    // A mutation triggers an immediate refresh that succeeds → recovery.
    await options.onInsert({
      transaction: { mutations: [{ modified: { id: 'a' } }] },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(onRecovered).toHaveBeenCalled();

    // Another mutation immediately after fails with the SAME message. Without
    // resetting the debounce on recovery this would be swallowed; it must not.
    await options.onInsert({
      transaction: { mutations: [{ modified: { id: 'b' } }] },
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(onError).toHaveBeenCalledTimes(2);
  });
});
