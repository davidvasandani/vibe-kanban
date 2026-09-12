<!--
SpecKit project constitution.
Edit this to capture the non-negotiable principles every feature must honor.
The Specify / Plan / Analyze stages read this file and check work against it.
-->

# Project Constitution

## Core Principles

### I. Clarity over cleverness
Code and specs are written to be read. Prefer the obvious solution; justify any
non-obvious one in the spec.

### II. Test the contract
Every feature defines how we will know it works (acceptance criteria) before it
is implemented. No feature is "done" without a checkable validation.

### III. Small, reversible steps
Ship the smallest change that delivers value. Avoid speculative generality.

### IV. One MCP contract for all agents
Shared MCP server configuration has one canonical definition and is adapted to
each agent's native config format at the boundary. Agent-specific exceptions must
be documented in the spec, preserve the shared behavior, and include validation
for each affected agent.

### V. Settings host scope is a data boundary
Host-specific Settings features must bind reads, writes, cache keys, and draft
state to the Settings-selected host. Specs must identify whether a Settings
section is local-only, remote-only, or host-scoped, and host-scoped work must
validate that switching hosts cannot show stale data or mutate the wrong
machine.

### VI. Responsive layout state owns layout chrome
Controls that are present only for a desktop, mobile, local, remote, or other
layout mode must use that mode's canonical state as the visibility authority.
Secondary environment or device signals may be retained as defense in depth, but
they must not contradict the selected layout. Specs that alter layout-specific
chrome must identify the owning layout signal and validate mismatches between
layout state and environment detection.

### VII. Bundled executable entries install what they advertise
Bundled catalog entries that cause a user's machine to execute third-party or
forked code must make provenance explicit and testable. The executable source
must match the advertised metadata, use an immutable reviewed version instead of
a moving branch or `latest` when a fork or non-obvious build is required, and
record the verification or audit control that keeps the pin honest. Specs that
change such entries must update the catalog, source constants, dependency
automation, and user documentation together, or record a deliberate exception
with its threat, detection path, and reopening trigger.

## Constraints
- Follow the existing architecture and conventions of the repository.
- Do not introduce new top-level dependencies without recording the reason in
  the plan's research notes.

## Governance
This constitution supersedes ad-hoc preferences. When a spec or plan conflicts
with it, the constitution wins or the conflict is recorded as an open question.

**Version**: 0.5.0
