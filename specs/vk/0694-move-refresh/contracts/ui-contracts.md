# UI contracts: Move deployment refresh

## RightSidebar

`RightSidebarProps` adds:

- `deployUpdateAvailable?: boolean` (default `false`)
- `onDeployRefresh?: () => void`

When `showDeployStatus` is false, neither status nor refresh renders. When true,
Deploy Status is the first collapsible section. Refresh renders only when
`deployUpdateAvailable` is true and a refresh callback is supplied.

## CollapsibleSectionHeader action

The existing `SectionAction` supports an explicit label used for accessible
name/title and, if the chosen implementation requires it, visible compact text.
Mouse click, Enter, and Space invoke the callback without toggling the section.

## AppBar

The existing props remain source-compatible during this change. Rendering
changes as follows:

- `updateVersion` truthy: render native Update and invoke `onUpdateClick`.
- `deployUpdateAvailable` truthy with no native update: render no AppBar Refresh.
- `appVersion` with neither update: render no AppBar revision.
