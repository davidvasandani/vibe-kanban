# Data Model: MCP Identifier and Display Label

## Logical shared server

`SharedMcpServer`

- `name: String` — protocol identifier/native map key; retained for API
  compatibility.
- `display_name: Option<String>` — Vibe Kanban presentation metadata. `None`
  means render `name`.
- existing definition, assignments, source, compatibility, auth, and gateway
  fields are unchanged.

`SharedMcpServerInput`

- `name: String` — submitted protocol identifier.
- `display_name: Option<String>` — submitted presentation metadata; blank or
  identical-to-identifier values normalize to `None`.
- existing definition, assignments, and native overrides are unchanged.

`SharedMcpConflict`

- `name: String` remains the conflicting native identifier.
- `display_name: Option<String>` is attached from the label store for
  presentation and carried into a selected variant.

## Label store

Versioned JSON document owned by Vibe Kanban:

```json
{
  "version": 1,
  "labels": {
    "atlassian_rovo": "Atlassian Rovo"
  }
}
```

Invariants:

- keys are safe protocol identifiers;
- values are trimmed, non-empty display labels different from their key;
- labels never affect native definition equality or fingerprints;
- stale labels for identifiers absent from every native profile are pruned only
  during a successful shared save, not during read;
- malformed metadata degrades to a scoped metadata error and does not block
  otherwise valid native agent configuration writes.

## Frontend draft

`SharedMcpDraftServer`

- `name: string` — protocol identifier.
- `displayName?: string | null` — presentation label.
- `definition`, `assignments` unchanged.

The stable key for all maps and actions is `name`. Rendering uses
`displayName?.trim() || name`.

## State transitions

1. Native read groups by native identifier.
2. Label store decorates matching logical servers/conflicts.
3. Catalog Add creates `{ name: catalog.key, displayName: catalog.name }`.
4. Unsafe existing Edit proposes a safe `name`, preserves original as
   `displayName`, and tracks the old name for removal in outer draft comparison.
5. Identifier and native-definition validation completes before any write;
   sidecar errors remain scoped metadata failures.
6. Native profile writes materialize definitions under `name` only.
7. After one or more relevant native successes, the label sidecar atomically
   converges to labels for the resulting logical set.
8. Reload repeats steps 1–2; tests/auth/actions continue to use `name`.
