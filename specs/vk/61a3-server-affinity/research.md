# Research Notes: Server Affinity Sidebar Polish (`61a3`)

## Existing data source

`RightSidebar.tsx` already finds the selected workspace in
`useWorkspaceContext().activeWorkspaces`. The summary's `serverAffinity` carries
the resolved hostname, requested hostname, and placement kind. This is the
correct collapsed-header source because the affinity body is unmounted when
closed and should not make a label-only request.

## Existing header contract

`CollapsibleSectionHeader` renders `headerExtra` alongside actions and the caret
for both states. Therefore the task does not need a new shared primitive or an
expanded-state callback. The feature caller must provide a bounded shrinkable
wrapper for dynamic hostname text.

## Spacing diagnosis

The affinity body currently uses vertical flex rows with
`justify-between gap-base`. In a wide drawer this pushes short labels to the far
left and the select to the far right, creating the large visual void shown in
the screenshot. A two-column grid keeps label/value relationships aligned while
still allowing the selector to contract at narrow widths.

## Alternatives rejected

- **Query placement in the header:** rejected because summary affinity exists,
  adds network/cache complexity, and can keep closed detail state alive.
- **Put the hostname into the section title string:** rejected because it harms
  semantic separation, localization, and independent truncation.
- **Change shared header spacing globally:** rejected because the defect is
  feature-specific and a global change would widen the blast radius to every
  collapsible section.
- **Fixed pixel widths for both columns:** rejected because localized labels and
  narrow sidebar widths need one flexible column.

## Dependency decision

No new dependency is needed.
