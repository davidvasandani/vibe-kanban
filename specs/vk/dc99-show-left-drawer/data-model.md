# Data model — useUiPreferencesStore (left drawer)

| Field | Persisted | Meaning |
|---|---|---|
| `isLeftSidebarVisible` | yes (`is_left_sidebar_visible`) | User intent only. Written by the toggle, by `setLeftSidebarVisible` and by the restore-on-close effect. Opening or closing a right panel no longer writes it. |
| `leftSidebarHasSpace` | no (default `true`) | Last fit result published by `WorkspacesLayout`. Reset to `true` when the layout unmounts. |
| `isLeftSidebarForcedOpen` | no (default `false`) | "Show anyway" override set by the toggle while the drawer is hidden for space. Cleared when `leftSidebarHasSpace` changes value or the drawer is hidden. |

Derived: `shown = isLeftSidebarVisible && (leftSidebarHasSpace || isLeftSidebarForcedOpen)`.

The `toggleLeftSidebar` transitions:

| Before | After |
|---|---|
| shown | `isLeftSidebarVisible=false`, `isLeftSidebarForcedOpen=false` |
| hidden, `isLeftSidebarVisible=false` | `isLeftSidebarVisible=true`, `isLeftSidebarForcedOpen=!leftSidebarHasSpace` (so one toggle always shows it) |
| hidden for space (`isLeftSidebarVisible=true`, no space) | `isLeftSidebarForcedOpen=true` |

Every toggle therefore flips what is on screen.
