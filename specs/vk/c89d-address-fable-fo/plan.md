# Implementation Plan: Preserve Preview App Navigation URLs

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The shared React/TypeScript preview surface lives in
`packages/web-core/src/pages/workspaces/PreviewBrowserContainer.tsx` and is used
by both local and remote frontends. The preview proxy injects
`crates/preview-proxy/src/devtools_script.js`, which reports complete
`location.href` navigation events. Per-workspace preview settings are stored as a
typed scratch payload defined by `PreviewSettingsData` in
`crates/db/src/models/scratch.rs` and accessed through
`packages/web-core/src/shared/hooks/usePreviewSettings.ts`. Rust-to-TypeScript
contracts are generated into `shared/types.ts`; that file must not be hand-edited.

The change must preserve existing URL override semantics, debounce durable
writes, keep scratch updates merge-safe, and include URL fragments when building
the proxy iframe URL.

## Architecture & Approach

1. Extend `PreviewSettingsData` with an optional `current_route` field. It stores
   only the latest route components (`pathname + search + hash`) reported for an
   auto-detected preview, leaving `url` as the existing full manual override.
   Optional/default deserialization preserves compatibility with existing scratch
   documents.
2. Regenerate `shared/types.ts` using `pnpm run generate-types`, so both frontend
   modes receive the same contract.
3. Extend `usePreviewSettings` to expose `currentRoute` and a durable
   `setCurrentRoute` operation. Every settings write carries forward all existing
   fields, including the new route, so screen-size and URL updates cannot erase
   it. Clearing a manual override must clear only the override field while
   retaining display settings and the separately tracked auto-detected route;
   deleting the whole scratch object is no longer safe once it has independent
   state.
4. Extract small pure URL helpers from `PreviewBrowserContainer.tsx` where that
   improves focused testing. Derive an auto-detected effective URL by applying a
   valid retained route to the freshly detected application origin. A manual
   override remains authoritative and is never rebased.
5. After `usePreviewNavigation` accepts a navigation event, canonicalize the
   displayed development URL, remove preview-only metadata, and persist only its
   route when the preview is auto-detected. An equality guard avoids write loops
   and redundant scratch traffic; route writes are immediate so leaving the
   preview cannot cancel the latest selection.
6. Include the URL fragment when constructing the proxy iframe source. Fragments
   never reach the proxy server, but they must be present in the browser-side
   iframe URL for hash-routed applications such as Mad Minutes.
7. Add Vitest regression coverage for route extraction/rebasing and proxy URL
   construction, including pathname, query, fragment, transport metadata, manual
   overrides, and changed ports. Add Rust serde compatibility coverage if the
   model currently has a suitable local test module.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/preview-settings.md` for the internal shared hook contract. No
new HTTP endpoint is required; the existing scratch API transports the expanded
payload.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- Principle II: focused tests verify the user-visible persistence contract.
- Principles III and VI: the design extends the existing scratch and bridge
  machinery with one optional field and pure URL helpers.
- Principle IV: behavior remains in `web-core`, so local and remote frontends
  share it.
- Principle XXI: existing proxy-to-development normalization remains canonical;
  route persistence calls it rather than defining a competing origin format.
- Generated types are regenerated through the mandated script.

No constitution deviation or unresolved question remains.

## Risks & Dependencies

- Scratch update callbacks can capture stale values. Tests and merge-complete
  writes must ensure one debounced update does not erase another field.
- Direct non-loopback previews lack the injected bridge, so their current route
  cannot be learned automatically; existing explicit override behavior remains
  their supported persistence path.
- A route from an unrelated app could be applied after the dev server changes.
  Scoping by workspace and applying it only to auto-detected previews is the
  intended compatibility boundary from clarification.
- Verification depends on installing the locked pnpm dependency graph in this
  fresh worktree.
