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
`packages/ui` presentational components own their own layout and section order;
containers in `web-core` only supply data via render props. A change to a shared
`packages/ui` component affects both local-web and remote-web — treat both
frontends as the blast radius.

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

### VII. Managed tools are pinned, verified, and user-owned
Managed CLI catalog entries are a supply-chain boundary. Each tool must have a
stable wire identifier, deterministic install location, pinned version, official
source link, platform-specific artifact mapping, exact SHA-256 verification, and
clear unsupported-host behavior. Installs and updates must use the existing
staged atomic workflow so failed downloads, checksum mismatches, or extraction
errors never leave partial executables on an agent's PATH. Tool credentials and
configuration remain user/host managed unless a spec explicitly and safely
defines otherwise.

## Constraints
- Follow the existing architecture and conventions of the repository.
- Do not introduce new top-level dependencies without recording the reason in
  the plan's research notes.
- Generated files (`shared/types.ts`, `shared/remote-types.ts`) are never edited
  by hand; regenerate via the `generate-types` scripts.
- Managed CLI catalog additions must preserve existing wire identifiers,
  host-copy precedence, removal/update behavior, and spawned-agent PATH
  semantics.
- Run `pnpm run format` before completing a task.

## Governance
This constitution supersedes ad-hoc preferences. When a spec or plan conflicts
with it, the constitution wins or the conflict is recorded as an open question.

**Version**: 0.4.0 (adds managed CLI supply-chain and host-ownership principle)
