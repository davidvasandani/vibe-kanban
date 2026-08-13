# Research: Concatenate Repeating Lines

## Visible-row owner

The screenshots show normalized tool rows whose content is
`codex review --uncommitted`. They are produced by the Codex command paths in
`crates/executors/src/executors/codex/normalize_logs.rs`, not by the Claude/Grok
compactor or the frontend.

## Existing pattern

`ClaudeLogProcessor` already collapses high-frequency system events and repeated
successful Grok `Bash` calls. The reusable invariants are:

- allocate once, then replace the original normalized index;
- prove adjacency with `EntryIndexProvider::current() == index + 1`;
- correlate raw lifecycle events by unique call ID;
- let only the latest occurrence own shared-row updates;
- require success before reuse;
- bound tick rendering.

The Codex normalizer has equivalent `CommandState` lifecycle maps and accepts two
event formats, but currently allocates a new entry at every command start.

## Scope decision

Generic identical-command compaction was rejected. Repeating a shell command can
be deliberate and each output can matter. The feature recognizes only the
reported `codex review --uncommitted` operation after the existing shell
unwrapping used for display.

Frontend compaction was rejected because it would duplicate stateful
tool-lifecycle logic, leave other consumers inconsistent, and receive patches
whose indices still include the hidden rows.

## Dependencies

No new dependency is needed. Existing shell parsing, normalized patch helpers,
`HashMap`, and `EntryIndexProvider` cover the implementation.
