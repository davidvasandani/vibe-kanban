# Spec: Electric sync timeout should fall back silently, not surface a user error

Task: `vk/a96d-electric-sync-er`

## Problem

When the remote (cloud) frontend subscribes to an ElectricSQL shape, the client
uses a **hybrid sync** (`packages/web-core/src/shared/lib/electric/collections.ts`):

1. It first tries to sync via ElectricSQL (`createHybridSync` → `electricSync`).
2. If Electric does not become "ready" within `ELECTRIC_READY_TIMEOUT_MS`
   (3000ms), the source is **locked to a REST fallback** that polls the
   `fallbackUrl` every 30s and applies snapshots. This is a deliberate,
   graceful degradation path — data keeps flowing to the UI.

The bug: the timeout handler reports the transition as a **sync error**:

```
Electric sync timed out after 3000ms, switching to fallback
```

`reportError` flows through `config.onError` → `useShape`'s `setError` →
`SyncErrorContext` → the navbar sync-error banner (`NavbarContainer` /
`RemoteWorkspaceRail`). So the user is shown a red, alarming error even though:

- the fallback is working and data is present, and
- nothing ever clears the error once the fallback recovers — the local `error`
  state in `useShape` is only reset by an explicit `retry()` or a tab
  visibility change, not by a successful fallback load. The banner therefore
  sticks around indefinitely on a healthy, fully-functional app.

The same "stale banner after recovery" problem also affects the other
Electric-unavailable paths (network fetch failure, HTTP 5xx) which both report
an error *and* lock to the fallback: once the fallback starts serving data the
error is stale, but nothing clears it.

## Goals

1. **The Electric-ready timeout must not surface a user-facing sync error.**
   Falling back to REST is expected recovery, not a failure. Log it (dev
   console) and switch silently.
2. **A successful fallback load must clear any prior sync error for that
   source.** Once the app has recovered (via any path that ended in a working
   fallback), the banner must disappear.
3. A genuine, unrecovered failure — Electric *and* fallback both failing — must
   still surface an error to the user (unchanged behaviour).

## Non-goals

- Changing the 3000ms timeout value or the 30s fallback poll interval.
- Changing the backend / Electric proxy or shape definitions.
- Reworking the `SyncErrorProvider` / banner UI.
- Changing mutation handling.

## Approach

All changes are in the frontend hybrid-sync layer.

### 1. Timeout → silent fallback (`collections.ts`)

In `createHybridSync`'s `scheduleReadyTimeout`, when the timeout fires with the
collection still not ready, replace the `reportError({ message: 'Electric sync
timed out …' })` call with a `console.warn` (the handler already guarantees the
page is visible at this point), then `lockSourceToFallback` as before. No
user-facing error is emitted for this transition.

### 2. Clear error on fallback recovery (`collections.ts`)

- Add a `reportRecovered` reporter derived from `config.onRecovered` in
  `createShapeCollection`.
- Thread it into `createHybridSync` → `createFallbackSync`.
- After a fallback fetch succeeds and `applySnapshot` runs in `refreshNow`,
  invoke `reportRecovered()`.

### 3. Wire it in `useShape` (`shared/integrations/electric/hooks.ts`)

Pass `onRecovered: handleRecovered` in the collection config, where
`handleRecovered` is a stable `useCallback` that clears local `error` state.
The existing effect then clears the `SyncErrorContext` entry for the stream.

### 4. Type update (`shared/lib/electric/types.ts`)

Add `onRecovered?: () => void` to `CollectionConfig`.

## Behavioural outcomes

| Scenario | Before | After |
| --- | --- | --- |
| Electric slow, fallback works | Persistent error banner | No banner; `console.warn` only |
| Electric errors (5xx/network), fallback works | Persistent error banner | Banner shows briefly, cleared once fallback loads |
| Electric errors, fallback also fails | Error banner | Error banner (unchanged) |

## Testing

- Add a Vitest unit test for `collections.ts` (mocking the TanStack DB layer,
  auth runtime, and remote API; stubbing `document`/timers) that asserts:
  - after the ready timeout with no Electric readiness, `onError` is **not**
    called, and the fallback fetch runs and applies rows;
  - a successful fallback load invokes `onRecovered`.
- `pnpm run check` and `pnpm run lint` pass.

## Risk

Low. Change is confined to the frontend hybrid-sync layer. The collection cache
means only the first `config` per collection id is used, but the reporters are
stable callbacks (as `onError` already is), so behaviour is unchanged there.
