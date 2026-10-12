# Prior knowledge: show the left drawer until there is no space

Task `vk/dc99-show-left-drawer`. Distilled from `wiki/` (read-only search for
sidebar / drawer / resizable / scratch / panel).

## Relevant pages

- **settings-drawer.md** — The Settings drawer docks by having the app shells
  (`SharedAppLayout`, `RemoteAppShell`) reserve its width as `marginRight`.
  Its dock-or-sheet decision is explicit **space math** (rail + 300px
  workspaces sidebar + a chat column; sheet below 1240px), and it notes that
  "the `md` breakpoint alone is not enough, because the viewport does not
  shrink when the shell does". While Settings is open, `WorkspacesLayout`
  hides its 300px right sidebar without touching the saved preference.
  → Measure the workspace area itself, not `window.innerWidth`; the Settings
  inset is then already excluded. Hiding chrome for layout reasons must not
  write the user's preference — the same rule this task applies to the left
  drawer.
- **workspace-sidebar-filtering.md** — "The preference scratch is typed on
  the server": `UiPreferencesData` (`crates/db/src/models/scratch.rs`) has no
  catch-all, so frontend-only fields are silently dropped on save. → New
  transient store fields (fit result, override) are deliberately *not*
  persisted; no Rust/scratch change is needed. `is_left_sidebar_visible`
  already exists and keeps its meaning (user intent).
- **flexible-collapsible-panel-stacks.md** — The right drawer
  (`RightSidebar.tsx`) is a fixed 300px column shared with the mobile `git`
  tab. → Its width is a constant we can use in the fit formula.
- **appbar-rail-and-org-tiles.md** — The app rail lives in `SharedAppLayout`
  to the left of the route content, so it is outside the measured workspace
  area.

## Not covered by the knowledge base

Nothing records the `isWideScreen()` (`> 2048px`) auto-collapse in
`useUiPreferencesStore`, the hover reopen tag/preview
(`useWorkspaceSidebarPreviewController`), or the ordering hazard between the
"restore left drawer when right panel closes" effect and scratch hydration.
These will be recorded after implementation.
