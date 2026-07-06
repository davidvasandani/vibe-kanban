# Client Electric hybrid sync & REST fallback

How the remote/cloud frontend syncs ElectricSQL shapes, what happens when
Electric is unavailable, and the design principle behind when a sync problem is
(and isn't) a user-facing error. Core file:
`packages/web-core/src/shared/lib/electric/collections.ts`.

## Architecture: Electric-first, REST fallback

`createShapeCollection` wraps a TanStack DB collection whose `sync` is a
**hybrid sync** (`createHybridSync`):

1. It first runs the real ElectricSQL sync (`electricSync`).
2. It arms a readiness timeout (`ELECTRIC_READY_TIMEOUT_MS`, 3000ms). If the
   collection has not become ready by then — or Electric errors with a network
   failure / HTTP 5xx (`onElectricUnavailable`) — the **source is locked to a
   REST fallback** (`lockSourceToFallback` → `createFallbackSync`).
3. The fallback polls the shape's `fallbackUrl` every
   `FALLBACK_REFRESH_INTERVAL_MS` (30s), fetches a full snapshot, and applies it
   via `applySnapshot` (truncate + re-insert + `markReady`).

The lock is per **source** (a `sourceKey` = table + sorted params), tracked in
the module-level `sourceRuntimes` map. Once a source is `fallbackLocked`, every
collection built for it — current and future — goes straight to the fallback and
skips the Electric attempt entirely. `registerFallbackSwitcher` lets already-live
Electric collections switch mid-flight when the lock flips.

## How a sync problem reaches (and leaves) the UI

The error path is a chain of callbacks, not a store:

```
reportError(SyncError)
  → CollectionConfig.onError            (collections.ts)
  → useShape setError(err)              (integrations/electric/hooks.ts)
  → SyncErrorContext.registerError      (providers/SyncErrorProvider.tsx)
  → navbar sync-error banner            (NavbarContainer / RemoteWorkspaceRail)
```

**Gotcha — errors do not auto-clear.** `useShape`'s local `error` state is only
reset by an explicit `retry()` or a tab visibility change. There is no
"sync succeeded, clear the error" signal built into the collection. So anything
that reports an error and then *recovers* will leave a stale banner up on a
fully-working app unless recovery is reported explicitly.

The recovery counterpart mirrors the error chain:

```
reportRecovered()
  → CollectionConfig.onRecovered
  → useShape setError(null)  (which clears the SyncErrorContext entry via effect)
```

`createFallbackSync` calls `reportRecovered()` right after a successful
`applySnapshot`, so once the fallback is actually serving data the banner clears.

**Recovery must also reset the error-report debounce.** `createErrorReporter`
holds an `ErrorHandler` that suppresses duplicate messages within an
exponentially-growing window (up to 30s). If recovery only cleared the banner
but left that state, a *fresh* failure with the same message shortly after a
recovery would be debounced away — banner cleared, no new banner, but the
fallback is actively failing. So `reportError` and `reportRecovered` are built
from **one** `createErrorReporter(config)` (a single shared handler), and
`reportRecovered` calls `handler.reset()` before `config.onRecovered()`. The
realistic trigger for sub-30s repeat failures is mutation-driven refreshes
(`maybeRefreshFallbackAfterMutation`), not the 30s poll.

## Principle: falling back is recovery, not an error

The 3000ms readiness timeout is **expected graceful degradation**, not a
failure. It must *not* surface a user-facing sync error — it only `console.warn`s
and locks to the fallback. A `SyncError` should reach the user **only when the
app genuinely has no working data source**, i.e. Electric *and* the fallback both
fail (`createFallbackSync`'s catch still calls `reportError`).

Behaviour contract (verified by `collections.test.ts`):

| Scenario | User-facing error? |
| --- | --- |
| Electric slow → fallback works | No (console.warn only), banner cleared via `onRecovered` |
| Electric 5xx/network → fallback works | Error shown briefly, then cleared once fallback loads |
| Electric down AND fallback down | Yes (unchanged) |

## Gotchas for future changes

- **Collections are cached by `collectionId`** (`collectionCache`); only the
  **first** `config` passed for a given id is used. Reporters derived from
  `config` (`onError`, `onRecovered`) must be **stable callbacks** on the
  `useShape` side (they are `useCallback`s) or a later render's handler will be
  silently ignored.
- **Not every consumer uses `useShape`.** `useAllOrganizationProjects.ts` calls
  `createShapeCollection` directly with no `config`, so `onError`/`onRecovered`
  are no-ops there. Keep `CollectionConfig` fields optional.
- **Testing the hybrid sync** (see `collections.test.ts`): mock
  `@tanstack/react-db`'s `createCollection` to capture the wrapped options and
  invoke `options.sync.sync(fakeSyncParams)` yourself; mock
  `electricCollectionOptions` to return an Electric sync that never calls
  `markReady` (forcing the timeout); stub `globalThis.document` (node env has no
  `document`, and the timeout handler reads `visibilityState`); drive time with
  `vi.advanceTimersByTimeAsync(3000)` (not `runAllTimers` — the 30s fallback
  `setInterval` never ends). Use a unique table/params per test so the
  module-level caches don't leak between cases.

## Contributed by

- vk/a96d-electric-sync-er
