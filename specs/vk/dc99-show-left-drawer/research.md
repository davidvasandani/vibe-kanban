# Research — vk/dc99-show-left-drawer

## Decision: measure the workspace root, not the window
`window.innerWidth` includes the app rail (~130px with org and project
columns) and the docked Settings inset, and it says nothing about which panels
are open. The Settings drawer page in the wiki already rejected viewport
breakpoints for the same reason. A `ResizeObserver` on the `WorkspacesLayout`
root sees exactly the space the drawer competes for.

## Decision: derive, don't persist
The old code stored the automatic collapse in the persisted preference. That
is why the drawer stayed hidden after resizing and after reloads. Keeping the
fit result in transient store fields means it is recomputed on every load and
resize.

## Decision: the store holds the fit result, not just layout-local state
`SharedAppLayout` (the hover reopen tag) and the action system
(label/isActive) live outside `WorkspacesLayout`. Both need the same "shown"
answer, so it goes in the same store as the preference.

## Rejected: react-resizable-panels collapsible panel for the drawer
That would make the drawer a percentage-based Panel inside a new outer Group.
It would change its fixed width and its resize behaviour, and it would make
"no space" a min-size violation rather than a hide. It is too much change for
this request.

## Rejected: CSS container query to hide the drawer
CSS can hide the element, but the hover tag, the action label and the toggle
override all need the result in JS.

## Rejected: `useIsMobile`-style matchMedia breakpoints
The breakpoint would have to be computed from the panel state anyway, and it
still includes the rail and the Settings inset.
