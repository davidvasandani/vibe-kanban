# Research

- `suggested_server_identifier` already implements the desired canonical form.
- `unchanged_legacy_server` exists only to permit no-op saves; it does not repair
  persisted state.
- Display labels already have a recoverable sidecar and must remain outside
  native definitions.
- Native profile writes are individually atomic but multi-profile writes can
  partially succeed today; this migration requires complete preflight and a
  rollback/recovery contract for the affected rename set.
