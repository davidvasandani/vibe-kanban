# Implementation plan — vk/4643-move-settings-to

See `SPEC.md` and `PRIOR_KNOWLEDGE.md`.

1. **Store.** Add `packages/web-core/src/shared/stores/useSettingsDrawerStore.ts`
   (zustand). It holds:
   - `isOpen`, `width`, `requestClose`
   - `setOpen`, `setWidth` (clamped, persisted to
     `localStorage['vibe.ui.settingsDrawerWidth']`), `registerCloseRequest`
   - `clampSettingsDrawerWidth(width, viewport)`, exported
   - `useSettingsDrawerInset(isMobile)`
   - Vitest: `useSettingsDrawerStore.test.ts`
2. **Drawer rendering.** Rewrite the chrome of `SettingsDialogContent` in
   `SettingsDialog.tsx`:
   - Remove the overlay.
   - On desktop, render a fixed right drawer at `z-[90]` with a left-edge
     resize handle (pointer drag) and `border-l`. On mobile, keep the
     full-screen sheet.
   - Narrow the nav column to `md:w-48`.
   - Scope Escape to the drawer (`onKeyDown` on the drawer, skipping
     `defaultPrevented`).
   - Register `handleCloseWithConfirmation` as the store's `requestClose`.
     Set `isOpen` on mount and clear it on unmount.
   - React to changes in the `initialSection`/`initialState` props (re-show
     while open).
3. **Toggle.** Export `toggleSettingsDrawer()` from `SettingsDialog.tsx`.
   `Actions.Settings.execute` calls it, and `isActive` reads
   `ctx.isSettingsOpen`. Add `isSettingsOpen` to `ActionVisibilityContext` and
   its type. Point the remote gear handlers (`RemoteAppShell`,
   `RemoteNavbarContainer`) and the no-arg remote action override at the
   toggle.
4. **Layout inset.** Apply `marginRight: useSettingsDrawerInset()` (with a
   width transition) to the desktop root of `SharedAppLayout.tsx` and
   `RemoteAppShell.tsx`.
5. **Verify.** Run web-core vitest, `pnpm run check`, `pnpm run lint` and
   `pnpm run format`. Then do a manual browser check with the local dev server
   if one is available.
6. **Review, knowledge base, PR.** Get a Codex review, update the wiki page,
   then open the PR and merge it.
