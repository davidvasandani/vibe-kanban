# Contracts: Timestamp the Workspace Chat Log

No network contract changes are required.

## Presentation contract

The shared workspace-chat renderer accepts the existing `DisplayEntry` and:

1. selects the atomic entry timestamp or latest valid aggregate member
   timestamp;
2. omits timestamp metadata for absent/invalid values and synthetic non-log
   controls;
3. emits semantic `<time dateTime="SOURCE_VALUE">COMPACT_LABEL</time>` metadata;
4. provides `FULL_LABEL` as hover/accessibility detail;
5. does not change the row's semantic key, ordering, process ownership, or
   interactive child component.
