# Data Model: Right Drawer Flexible Sections

No persisted domain data, API payload, database schema, or generated type
changes are required.

The only transient state is the existing boolean expanded state already owned
by `CollapsibleSectionHeader`. The feature derives presentation classes from
that state and introduces no additional state.
