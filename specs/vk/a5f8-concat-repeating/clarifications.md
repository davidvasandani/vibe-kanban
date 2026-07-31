# Clarifications: Concatenate Repeating Lines

## Resolved Decisions

### Eligible command

Compaction is limited to the reported `codex review --uncommitted` operation.
An absolute executable path or shell wrapper is acceptable when the command
normalizes to that same visible operation. Other review targets and arbitrary
identical shell commands remain distinct.

This is the smallest scope supported by the screenshots. It also follows prior
project knowledge that generic command suppression can conceal deliberate
repeated operations and their outputs.

### Marker meaning

The marker counts successful repetitions after the first occurrence, matching
the existing repeated-log convention. Thus three total successful executions
render `✓✓`, while ten total executions render `✓ ×9`.

### Completion and failure boundary

Only a successfully completed occurrence arms reuse for the next matching
command. Failed, denied, or timed-out occurrences remain visibly unsuccessful
and disarm the run. A later matching command starts a new row.

### Protocol coverage

Both current app-server item notifications and the legacy Codex event stream
must use the same compaction rules because Vibe Kanban accepts both formats in
one normalizer.

## Remaining Open Questions

None.
