<!--
SpecKit project constitution (vibe-kanban).
The Specify / Plan / Analyze stages read this file and check work against it.
-->

# Project Constitution — vibe-kanban

## Core Principles

### I. Clarity over cleverness
Code and specs are written to be read. Prefer the obvious solution; match the
comment density, naming, and idiom of the surrounding code. Justify any
non-obvious choice in the spec or plan.

### II. Test the contract
Every feature defines how we will know it works (acceptance criteria) before it
is implemented. Rust logic gets `#[cfg(test)]` unit tests; UI/section changes
get a rendered-DOM component test where one already exists for that surface. No
feature is "done" without a checkable validation.

### III. Small, reversible steps
Ship the smallest change that delivers value. Prefer reusing an existing
component and an existing data source over adding new plumbing. When behaviour
already exists for one case, generalise it rather than duplicating it. Avoid
speculative generality.

### IV. Shared-component boundaries are law
The frontend has two shared tiers: `packages/ui` (`@vibe/ui`) owns primitive
presentational components (Button, Dialog, Badge, etc.) and their own internal
layout; `packages/web-core` owns shared feature containers and data-fetching
logic used by both local-web and remote-web. Containers in `web-core` supply
data to `packages/ui` primitives; they do not reimplement presentation.
A change to either shared package affects both `local-web` and `remote-web` —
treat both frontends as the blast radius. Styling guidance lives in
`packages/local-web/AGENTS.md`.

### V. Remote mutations are transactional and txid-covered
On the `crates/remote` server the read path is ElectricSQL shapes; the write
path is REST handlers. Every mutation runs its DB work inside one Postgres
transaction and returns the `txid` the client waits on before dropping optimistic
state. Any side effect triggered by a mutation (e.g. archiving workspaces when an
issue's status changes) MUST run on the same transaction connection as the
triggering write, so it commits atomically and is covered by that one `txid`.
Per-project `project_statuses` are user-customisable and carry no terminal flag;
existing code identifies terminal statuses ("Done", "Cancelled") by
case-insensitive name — follow that established convention rather than inventing a
new category concept.

### VI. Don't rebuild what shipped
Extend existing machinery instead of forking it. Before adding a code path,
search for one that already does the job (git history, the knowledge base, the
crate's `AGENTS.md`) and build on it.

### VII. Workspace breadcrumbs preserve issue identity
Any workspace breadcrumb that represents or links through an issue MUST display
the issue ID. The ID is the stable cross-view reference for task identity, so it
must remain visible even when the workspace title, issue title, or responsive
layout is shortened.

### VIII. Managed tools are pinned, verified, and user-owned
Managed CLI catalog entries are a supply-chain boundary. Each tool must have a
stable wire identifier, deterministic install location, pinned version, official
source link, platform-specific artifact mapping, exact SHA-256 verification, and
clear unsupported-host behavior. Installs and updates must use the existing
staged atomic workflow so failed downloads, checksum mismatches, or extraction
errors never leave partial executables on an agent's PATH. Tool credentials and
configuration remain user/host managed unless a spec explicitly and safely
defines otherwise.

### IX. External agent protocols are defensive contracts
Coding-agent integrations use the vendor's documented noninteractive protocol,
preserve stable serialized executor identifiers, and parse structured output
without assuming the event schema is closed. Unknown events must degrade safely;
session identity, cancellation, failures, and credential redaction must remain
correct. Extend the shared executor, log-normalization, profile, and MCP
abstractions before introducing agent-specific parallel machinery.

### X. Dialogs hold provisional state; containers hold confirmed state
Settings dialogs and edit modals own a private snapshot of the data they mutate.
On open, the dialog is seeded from the current saved state (or blank for "add").
The dialog may freely mutate its own local copy. Only an explicit submit action
writes the complete, validated result back to the persistent draft or store.
Close and cancel must discard all modal-local changes without touching the outer
state. Inline mutations of shared draft state from inside an open form are
disallowed. This applies to MCP server definitions, agent assignments, and any
other compound form that edits a named object inside a larger collection.

### XI. Diagnostics are evidence, not decoration
Tool, executor, integration, and MCP diagnostics are user-facing evidence for
debugging. UIs that surface diagnostics MUST preserve the exact backend-provided
text for display, copy, and issue/task seeding, including line breaks and long
unbroken content, while treating that text as inert and untrusted. Diagnostic
actions may add surrounding context, but they must not truncate, reinterpret,
auto-remediate, or silently discard the original diagnostic.

## Constraints
- Follow the existing architecture and conventions of the repository.
- Do not introduce new top-level dependencies without recording the reason in
  the plan's research notes.
- Generated files (`shared/types.ts`, `shared/remote-types.ts`) are never edited
  by hand; regenerate via the `generate-types` scripts.
- Managed CLI catalog additions must preserve existing wire identifiers,
  host-copy precedence, removal/update behavior, and spawned-agent PATH
  semantics.
- Executor additions must include generated-contract regeneration, exhaustive
  backend/frontend mapping checks, and fixture-based protocol tests.
- Diagnostic issue/task creation must use an explicit current project or
  workspace context; never choose an arbitrary project when context is missing.
- Run `pnpm run format` before completing a task.

## Governance
This constitution supersedes ad-hoc preferences. When a spec or plan conflicts
with it, the constitution wins or the conflict is recorded as an open question.

**Version**: 0.8.0 (adds diagnostic fidelity principle XI and explicit-context
constraint for diagnostic issue/task creation)
