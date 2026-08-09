# Data Model: Scrollable Create-Issue Settings

No database entities, API DTOs, generated TypeScript types, persisted settings,
or React form-state fields change.

The existing `IssueFormData` and all create/edit values remain intact. This
feature changes only the layout constraints of two existing rendered elements:

| Element | Existing role | Required invariant |
| --- | --- | --- |
| Panel shell | Full-height column containing header and body | Constrains overflow and retains a fixed header |
| Panel body | Contains properties, editor, settings, create action, and edit sections | Can shrink below intrinsic content height and owns vertical scrolling |
