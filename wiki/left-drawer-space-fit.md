# Left drawer: shown until there is no space

The workspaces drawer (`WorkspacesSidebarContainer`, a fixed 300px column) is
on the left of `WorkspacesLayout`. It is on screen when the user wants it and
the layout has room for it. Before `vk/dc99-show-left-drawer`, any window up
to 2048px hid it when a right panel opened. This page records why the
replacement is shaped the way it is (constitution LIV).

## Three fields, one derived answer

In `useUiPreferencesStore`:

- **`isLeftSidebarVisible`**: the user's intent. It is persisted to the
  server scratch as `is_left_sidebar_visible`.
- **`leftSidebarHasSpace`**: transient. It is the last fit result published
  by `WorkspacesLayout`.
- **`isLeftSidebarForcedOpen`**: transient. It records "show anyway",
  requested while there is no space.

`selectIsLeftSidebarShown` combines the three:
`isLeftSidebarVisible && (leftSidebarHasSpace || isLeftSidebarForcedOpen)`.

Everything that cares whether the drawer is on screen uses that selector, and
`useWorkspacePanelState().isLeftSidebarVisible` already returns it:

- the layout render;
- the hover reopen tag and preview in `SharedAppLayout`;
- the `ToggleLeftSidebar` label and its `isActive` (through the action
  context).

Read the raw field only when you mean intent.

**Gotcha: never write layout decisions into `isLeftSidebarVisible`.** The old
`isWideScreen()` collapse did that. Because the field is persisted, an
automatic hide outlived resizes and reloads. The "restore drawer when the
right panel closes" effect in `WorkspacesLayout` only re-runs when its deps
change. So a scratch hydration that landed after mount could leave the drawer
hidden with nothing open, which is the state in the bug recording. Saved
`false` values from that era look identical to real choices and were left
alone. Closing a right panel, or one `V S`, restores the drawer.

## Measuring

- **The root is measured, not the window.** `WorkspacesLayout` puts a
  `ResizeObserver` on its desktop root, so the rail and the docked Settings
  inset are already excluded (see [settings-drawer](settings-drawer.md)).
- **The fit rule is stable.** The drawer is inside the measured root, so
  showing or hiding it does not change the reading and cannot oscillate.
- **The fit rule lives in `shared/lib/workspaceLayoutFit.ts`.** Only rendered
  panels count:
  - the drawer and the right sidebar are 300px each;
  - the chat needs at least 480px;
  - a right panel needs at least 400px.

  Chat plus right sidebar therefore needs 1080px of root width, and adding a
  right panel raises that to 1480px.
- **The right sidebar counts only when it is rendered:**
  `isRightSidebarVisible && !isCreateMode && !isSettingsDrawerOpen`.
- **Resize drags stay cheap.** The width lives in a ref, and only the boolean
  reaches the store, so a drag re-renders the layout only when the drawer
  flips. Width `null` (unmounted, or the mobile branch) publishes `true`.

## Toggle semantics

`toggleLeftSidebar` acts on what is on screen:

- **Drawer shown:** hide it, and save that.
- **Drawer hidden:** show it. When there is no space, also set the override,
  so one press always shows the drawer.
- **Override lifetime:** `setLeftSidebarHasSpace` clears the override only
  when the fit value actually changes. Repeated identical readings keep it.

## Verifying

- Store and fit logic: `useUiPreferencesStore.leftSidebar.test.ts` and
  `workspaceLayoutFit.test.ts`. The store is importable under Vitest's node
  environment if `window.matchMedia` is stubbed.
- Live resize without agent-browser: use `puppeteer-core` with nix
  `chromium`. Call `page.setViewport` at several widths and test for
  `div.w-\[300px\].shrink-0`. `/workspaces/create` works on a fresh dev
  database. Below 768px the mobile layout takes over.

## Contributed by

- vk/dc99-show-left-drawer
