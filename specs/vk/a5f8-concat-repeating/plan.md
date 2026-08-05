# Implementation Plan: MCP Identifier and Display-Label Separation

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust 2024, serde/serde_json, Tokio, ts-rs in
  `crates/executors/src/shared_mcp_config.rs` and server config routes.
- Frontend: React/TypeScript in `packages/web-core`, shared generated contracts
  in `shared/types.ts`.
- Storage: coding-agent-native files remain executable source of truth; a small
  Vibe Kanban-owned JSON sidecar stores display labels only.
- Constraints: no new dependency, no generated-file hand edits, no changes
  outside the Vibe Kanban service, and all identifiers validated before writes.

## Architecture & Approach

### 1. Backend label metadata

Add a private label-store module adjacent to shared MCP logic. Resolve its path
from the existing platform config directory with an overridable path seam for
tests. Reads are tolerant and return an empty map plus a scoped diagnostic on
failure. Writes validate/normalize labels, create the app directory, write a
same-directory temporary file with restrictive permissions, then rename.

Decorate `SharedMcpServer` and `SharedMcpConflict` after native grouping. Labels
never enter `canonical_definition`, fingerprints, native sources, assignment
identity, gateway identity, or materialization.

### 2. Write ordering

Extend inputs with optional display metadata and validate identifiers,
duplicates, assignments, compatibility, and label shape before writes. Keep the
existing per-profile native-write loop. If no native write succeeds, do not
change labels. If at least one succeeds (including a partial result), converge
the sidecar to the labels associated with servers known to exist after the
planned successful writes; surface a sidecar failure as a specific failed
outcome/status without rolling back already committed native files.

### 3. Frontend identity separation

Extend `SharedMcpDraftServer`, conversions, snapshots, JSON mode, conflict
promotion, and OAuth-refresh merge with `displayName`. Catalog parsing continues
to expose key/name; Add sends key as identifier and metadata name as label.

Extract frontend identifier helpers with parity tests against Rust cases.
`McpServerDialog` gets distinct Identifier and Display name fields. Unsafe
existing names seed the normalized candidate and preserve the original name as
label; the outer settings container retains the original identifier for removal
when the confirmed rename lands. Collision detection remains exact by identifier.

Cards render the friendly label as primary text and a monospace identifier as
secondary text when different. Audit every callback/state map so tests, OAuth,
gateway disconnect, refresh, edit/delete, checked-time, copy, and debug actions
continue to use `server.name`.

### 4. Tests and generated types

Backend tests cover normalization, label-store serialization/errors/atomic
replacement, DTO compatibility, decoration, pre-write collision rejection,
partial-failure ordering, and native materialization without label fields.
Frontend tests cover catalog mapping, draft/snapshot/JSON/OAuth merge, conflict
promotion, legacy unsafe edit seeding, collisions, and operational identifiers.
Regenerate `shared/types.ts`, then run focused suites and repository checks.

## Data Model

See [`data-model.md`](data-model.md).

## Contracts

See [`contracts.md`](contracts.md).

## Research Notes

See [`research.md`](research.md). No new dependency is introduced.

## Constitution Check

- II: explicit backend/frontend contract tests and acceptance exercise.
- III/VI: extends the existing shared MCP read/write pipeline and catalog split.
- X: rename/label edits remain modal-local until submit and outer Save.
- XIII: native vendor files retain atomic guest-editor behavior; labels live in
  app-owned metadata and never alter unrelated native fields.
- XVII: live refresh remains keyed by stable configured identifier.
- XXI: normalization has one named contract and parity fixtures.
- XXII: wire identity and presentation are separate; collisions precede writes;
  no silent native-key migration or definition metadata injection.

No constitution deviation is required.

## Risks & Dependencies

- Sidecar/native writes cannot be one filesystem transaction; truthful partial
  outcomes and reload from native truth mitigate this.
- A stale sidecar can outlive manual native edits; absent identifiers are ignored
  on read and pruned only during successful Save.
- Rename can affect OAuth identity. It is explicit and the UI must warn; gateway
  capability preservation remains URL-aware but tests must cover it.
- Shared type changes affect local and remote web consumers; full generated-type
  and frontend checks are required.
