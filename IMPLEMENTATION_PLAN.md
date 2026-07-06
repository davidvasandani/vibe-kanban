# Implementation Plan — `vk/a96d-electric-sync-er`

See `SPEC.md` for the full design. All changes are in `packages/web-core`.

## Step 1 — Type: add `onRecovered` to `CollectionConfig`

File: `packages/web-core/src/shared/lib/electric/types.ts`

Add an optional callback to `CollectionConfig`:

```ts
export interface CollectionConfig {
  /** Callback for sync errors */
  onError?: (error: SyncError) => void;
  /** Called when a source recovers (e.g. a fallback snapshot loads),
   *  so previously-reported errors can be cleared. */
  onRecovered?: () => void;
}
```

## Step 2 — `collections.ts`: silent timeout + recovery reporting

File: `packages/web-core/src/shared/lib/electric/collections.ts`

1. **Recovery reporter.** In `createShapeCollection`, alongside
   `const reportError = createErrorReporter(config);` add:
   ```ts
   const reportRecovered = () => config?.onRecovered?.();
   ```
   Pass `reportRecovered` into `createHybridSync({ ... })`.

2. **Thread through `createHybridSync`.** Add `reportRecovered: () => void` to
   its args type and forward it into the `createFallbackSync({ ... })` call it
   constructs.

3. **`createFallbackSync`.** Add `reportRecovered: () => void` to its args.
   In `refreshNow`, after a successful fetch + `applySnapshot(syncParams, rows)`
   (inside the `if (!isCleanedUp)` block), call `args.reportRecovered()`.

4. **Silent timeout.** In `createHybridSync`'s `scheduleReadyTimeout`, replace:
   ```ts
   args.reportError({
     message: `Electric sync timed out after ${ELECTRIC_READY_TIMEOUT_MS}ms, switching to fallback`,
   });
   lockSourceToFallback(args.sourceKey);
   ```
   with:
   ```ts
   console.warn(
     `Electric sync timed out after ${ELECTRIC_READY_TIMEOUT_MS}ms, switching to fallback`
   );
   lockSourceToFallback(args.sourceKey);
   ```
   (Page visibility is already guaranteed earlier in the handler.)

## Step 3 — `useShape`: clear error on recovery

File: `packages/web-core/src/shared/integrations/electric/hooks.ts`

Add a stable recovery handler and pass it in the collection config:

```ts
const handleRecovered = useCallback(() => setError(null), []);
// ...
const config = { onError: handleError, onRecovered: handleRecovered };
```

Add `handleRecovered` to the `useMemo` dependency list that builds the
collection. Setting `error` to `null` also clears the `SyncErrorContext` entry
via the existing effect keyed on `error`.

## Step 4 — Test

File: `packages/web-core/src/shared/lib/electric/collections.test.ts`

Vitest unit test. Mocks:
- `@tanstack/electric-db-collection` → `electricCollectionOptions` returns
  `{ id, sync: { sync: <electric sync that never markReady> } }` merged with
  input options.
- `@tanstack/react-db` → `createCollection` captures the passed options and
  synchronously invokes `options.sync.sync(fakeSyncParams)`, returning a stub.
- `@/shared/lib/auth/runtime` → `getAuthRuntime` stub.
- `@/shared/lib/remoteApi` → `getRemoteApiUrl`, `getRemoteApiBasicAuth`,
  `makeRequest` (records calls; returns `{ ok, json }` with fallback rows).

Harness: stub `globalThis.document = { visibilityState: 'visible' }` and use
`vi.useFakeTimers()`. `fakeSyncParams` implements
`collection.isReady/onFirstReady`, `begin/write/commit/markReady/truncate`.

Assertions:
1. Create a collection (unique table name), advance timers past 3000ms, flush
   promises. Expect: `onError` **not** called; `makeRequest` called with the
   fallback path; rows written + `markReady` called; `onRecovered` called.
2. (Optional secondary case) confirm the fallback poll uses the shape's
   `fallbackUrl`.

Use a unique `table`/`params` per test so the module-level `collectionCache`,
`sourceRuntimes`, and `fallbackSnapshotCache` don't leak between cases.

## Step 5 — Verify

- `pnpm --filter @vibe/web-core test` (or `pnpm run test` in the package).
- `pnpm run check` (frontend typecheck + Rust) and `pnpm run lint`.
- `pnpm run format`.

## Step 6 — Review & knowledge (pipeline stages 4–5)

- Codex review of the diff; address findings; re-verify.
- Add a `wiki/` page on the client Electric hybrid-sync + fallback design and
  the "fallback is recovery, not a user error" principle; tag
  `vk/a96d-electric-sync-er`; refresh `wiki/INDEX.md`; commit.

## Files touched

- `packages/web-core/src/shared/lib/electric/types.ts`
- `packages/web-core/src/shared/lib/electric/collections.ts`
- `packages/web-core/src/shared/integrations/electric/hooks.ts`
- `packages/web-core/src/shared/lib/electric/collections.test.ts` (new)
- `wiki/*` (stage 5)
