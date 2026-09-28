# Prior knowledge — vk/4643-move-settings-to

Sources searched: `wiki/INDEX.md` and its pages, plus
`docs/knowledge-base/INDEX.md` and its pages, for "settings", "drawer",
"modal" and "dialog". No page covers how the Settings surface itself is laid
out. The pages below are the closest matches.

## What applies

- **`wiki/flexible-collapsible-panel-stacks.md`** covers the workspace right
  drawer (`RightSidebar.tsx`):
  - `RightSidebar` is shared between the desktop drawer and the mobile `git`
    tab. Desktop-only chrome has to be opted into by the layout mount.
  - Every ancestor in a height-constrained stack needs `min-h-0`.
  - **Implication:** Settings should not go into `RightSidebar`. It is a fixed
    300px, workspace-only, mobile-shared stack, and Settings must open from
    every route. Build an app-shell-level drawer instead.
- **`docs/knowledge-base/nested-flex-scroll-containment.md`:** in a fixed
  header plus scrolling body, the shell is `flex flex-col h-full`, the header
  is `shrink-0`, and the body is `min-h-0 flex-1 overflow-y-auto`. The drawer
  gets a definite height from `fixed inset-y-0`. Keep the existing section
  scroll owner (the content column's `overflow-y-auto`).
- **`docs/knowledge-base/pipeline-settings-editor.md`** (Settings host
  switching):
  - Dirty-state confirmation is global across sections.
  - The host-scoped subtree is keyed by the selected host.
  - Drawer changes must keep `SettingsDirtyProvider` / `SettingsHostProvider`
    wrapping and the close-with-confirmation flow untouched.
- **`docs/knowledge-base/remote-machine-management.md`:** removing the route's
  active host closes Settings through the section's `onClose` prop. The drawer
  must keep passing `onClose` into sections.
- **`wiki/appbar-rail-and-org-tiles.md`:** the AppBar rail (remote) holds a
  settings gear. It calls `SettingsDialog.show()` like the other entry points,
  so as long as that API stays the same those entry points need no changes.

## Gotchas found while scouting the code

- The dialog/overlay z-layers in `@vibe/ui` are 9998/9999, and dropdowns are
  10000. A non-modal drawer should sit below 9998 so that the confirm dialogs
  and dropdowns opened from Settings still stack above it.
- `Scope.SETTINGS` in `keyboard/registry.ts` is declared but not used.
  Settings' Escape handling is a hand-rolled `window` listener, and a
  non-modal drawer has to scope it to focus inside the drawer.
- About 20 call sites use `SettingsDialog.show(...)`, and some of them `await`
  it. Keep the nice-modal API.
