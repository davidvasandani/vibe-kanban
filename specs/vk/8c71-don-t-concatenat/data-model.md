# Data Model: Single-Value Browser Titles

No persisted or domain data is introduced or changed.

The only transient inputs are an ordered list of optional strings. Selection
returns the trimmed first value containing a non-whitespace character, or the
constant `Vibe Kanban` when no such value exists.
