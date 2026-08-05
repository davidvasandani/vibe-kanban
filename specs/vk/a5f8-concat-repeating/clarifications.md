# Clarifications: MCP Identifier and Display-Label Separation

## Resolved Decisions

### Display labels survive independently of the live catalog

Display labels are Vibe Kanban-owned logical metadata and must persist across a
save/reload even if a catalog entry is later removed or renamed. They must not
be inserted into coding-agent-native MCP definition objects. The plan stage will
select the existing Vibe Kanban configuration/state mechanism that can hold this
metadata with the least new plumbing; it must not turn native executable
definitions into a second source of truth.

Reason: re-deriving from the current catalog makes the UI label mutable and
loses labels for MCPs originating from external catalogs or plugins—the reported
class of failure.

### Identifier collisions require an explicit user choice

When a suggested identifier is already in use, the UI opens the edit/add flow
with the safe candidate visible and reports that it is taken. Vibe Kanban does
not automatically append a numeric suffix because that creates a durable
external identifier without user confirmation and may obscure that the same MCP
is already configured.

### Unsafe existing native keys can be explicitly repaired

An unsafe native key remains unchanged merely by loading settings. Editing it
seeds a safe identifier suggestion and retains the original text as the display
label. Submitting and then saving is an explicit remove-plus-add rename, so the
user can repair the config without editing files externally. The UI warns that
the identifier is changing; cancel leaves the native key untouched.

### Identifier normalization

Normalization is ASCII and deterministic: trim surrounding whitespace,
lowercase ASCII alphanumerics, preserve `_` and `-`, replace each run of other
characters with one `_`, trim resulting leading/trailing underscores, and fall
back to `mcp_server` when nothing remains. This is the existing backend
suggestion contract and must have one equivalent frontend implementation or a
server-provided result, not independently drifting variants.

## Remaining Open Questions

None.
