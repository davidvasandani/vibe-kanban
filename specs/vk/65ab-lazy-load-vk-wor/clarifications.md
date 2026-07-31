# Clarifications: Lazy-load workspace chat history

## Resolved decisions

### Continuation scope

The user-facing continuation is conversation/session-level. Callers request the
previous page for the selected workspace session rather than coordinating
separate process cursors. The implementation may encode process-local position
inside the opaque cursor, but process boundaries are not exposed as paging UX.

This matches the visible transcript, which is one chronological conversation
assembled from many execution processes, and prevents undersized/oversized pages
when a boundary happens to fall between turns.

### Page unit and size

Pages are measured in final materialized normalized entries, with a default of
100 and a server-enforced maximum of 200. The server may return fewer entries
to preserve an indivisible semantic boundary, but must never exceed the maximum.

Normalized entries are the durable transport unit and retain add/replace
identity. Semantic display rows are frontend-derived and can aggregate several
entries, while complete turns can be arbitrarily large and would violate the
bounded-window requirement.

### Load trigger and accessibility

Support both: automatically request one page when the top sentinel becomes
visible, and expose an accessible load-earlier/retry control. Both triggers feed
the same single-flight action. Automatic loading provides expected chat UX; the
control covers keyboard, assistive-technology, observer failure, and retry.

### Script output

Raw setup, cleanup, and archive script logs remain on the existing raw-log
stream for this feature. They are process diagnostics, not normalized workspace
chat messages, and adding a second pagination contract would expand scope
without fixing the reported long-chat bottleneck.

## Remaining open questions

None.
