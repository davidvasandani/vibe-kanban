# Implementation plan: show the left drawer until there is no space

Task `vk/dc99-show-left-drawer`. Spec: `SPEC.md`; SpecKit artifacts in
`specs/vk/dc99-show-left-drawer/`.

1. **Pure fit helper** — new `packages/web-core/src/shared/lib/workspaceLayoutFit.ts`:
   constants `LEFT_SIDEBAR_WIDTH = 300`, `RIGHT_SIDEBAR_WIDTH = 300`,
   `MIN_CHAT_WIDTH = 480`, `MIN_RIGHT_MAIN_PANEL_WIDTH = 400`, and
   `leftSidebarFits({ width, isChatShown, isRightMainPanelOpen,
   isRightSidebarShown })`. Unit test it (`workspaceLayoutFit.test.ts`).
2. **Store** (`useUiPreferencesStore.ts`):
   - delete `isWideScreen`; stop writing `isLeftSidebarVisible` from
     `toggleRightMainPanelMode` / `setRightMainPanelMode`;
   - add transient `leftSidebarHasSpace` (default `true`) and
     `isLeftSidebarForcedOpen` (default `false`) plus
     `setLeftSidebarHasSpace(value)` (clears the override when the value
     changes);
   - export `selectIsLeftSidebarShown(state)` =
     `isLeftSidebarVisible && (leftSidebarHasSpace || isLeftSidebarForcedOpen)`;
   - `toggleLeftSidebar`: shown → preference false + override off; hidden by
     preference → preference true; hidden for space → override on;
   - `useWorkspacePanelState` returns the *shown* value as
     `isLeftSidebarVisible` (layout + action context follow automatically).
   - Do not touch `useUiPreferencesScratch` — new fields stay unpersisted.
3. **WorkspacesLayout** — callback ref + `ResizeObserver` on the desktop
   root to track its width; compute `leftSidebarFits` from the rendered
   panels (chat visible, right main panel open, right sidebar actually shown
   = `isRightSidebarVisible && !isCreateMode && !isSettingsDrawerOpen`);
   publish via `setLeftSidebarHasSpace`; reset to `true` on unmount. Keep the
   "restore drawer when right panel closes" effect.
4. **SharedAppLayout** — hover preview enabled when the drawer is not shown
   (`selectIsLeftSidebarShown`), so an auto-hidden drawer is still reachable.
5. **Action** — `ToggleLeftSidebar.label` uses `selectIsLeftSidebarShown`.
6. **Store tests** — `useUiPreferencesStore.leftSidebar.test.ts`: opening a
   right panel keeps the preference; toggle in all three states; override is
   cleared when space changes.
7. **Verify** — `pnpm install --frozen-lockfile`, web-core vitest,
   `pnpm run check`, `pnpm run lint`, `pnpm run format`; manual check in the
   dev app with the browser at several widths.
