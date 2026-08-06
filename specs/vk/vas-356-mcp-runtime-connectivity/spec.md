# Feature Specification: Cluster-safe MCP runtime connectivity

**Task:** VAS-356
**Scope:** Vibe Kanban worker runtime and explicitly authorized Firecrawl ingress

## User problem

An MCP can be saved, tested successfully by the coordinator, and still be
unusable by Codex because Codex runs on a cluster worker. `vibe_kanban` selects a
stale worker-local backend port and Firecrawl rejects worker source addresses.

## Functional requirements

- **FR-1:** Worker-launched processes receive the configured coordinator URL in
  the environment variable consumed by the bundled Vibe Kanban MCP.
- **FR-2:** The worker's existing coordinator variable remains unchanged and
  both values are derived from one Nix option.
- **FR-3:** Firecrawl port 3410 accepts the coordinator and every configured VK
  worker address.
- **FR-4:** Logmein ports 8189 and 8190 retain their current source allowlist.
- **FR-5:** Other source addresses remain denied for all three protected ports.
- **FR-6:** Focused evaluation prevents future drift in environment propagation
  and source-address scoping.

## Success criteria

- Evaluated worker service environments contain identical non-empty coordinator
  URLs under both variable names.
- Rendered think1 nftables rules widen only TCP 3410 to VK workers.
- Nix evaluation and repository checks pass.
- Following deployment, direct Firecrawl MCP initialization and a Vibe Kanban
  MCP API call succeed from a VK worker.

## Non-goals

- Changing Codex's pinned version or reload protocol.
- Making Firecrawl generally LAN-accessible.
- Reworking MCP catalog persistence or the Settings UI.

## Follow-up: authenticated definitions on remote workers

### User stories

- As a user who assigns an authenticated MCP to Codex, I want a workspace sent
  to a cluster worker to use the same definition so it behaves like a local run.
- As an operator, I want credentials to remain inside Vibe Kanban's established
  authenticated execution boundary rather than duplicate them in deployment
  configuration.

### Functional requirements

- **FR-7:** Remote execution must use the selected executor profile's
  coordinator-authoritative MCP server definitions.
- **FR-8:** A worker must receive and apply the MCP definitions before starting
  the coding-agent process.
- **FR-9:** Synchronization must preserve unrelated native executor settings.
- **FR-10:** Secret-bearing values must not appear in logs, errors, Git, Nix
  expressions, command arguments, or public API responses introduced by this
  feature.
- **FR-11:** A present but invalid, mismatched, or oversized snapshot must fail
  the dispatch before agent startup; an absent snapshot retains rolling-version
  compatibility.
- **FR-12:** Idempotent dispatch identity must cover the snapshot so a replay
  cannot apply different MCP credentials under the same request digest.

### Acceptance criteria

- A remote Codex execution has the same canonical MCP server map as coordinator
  Codex at dispatch time.
- Firecrawl scope bootstrap succeeds on think3/think4 using the bearer already
  stored in the coordinator's MCP definition.
- Tests cover protocol compatibility, executor mismatch, payload bounds,
  native-setting preservation, digest sensitivity, and secret-safe failures.

### Clarified decisions

- The coordinator snapshot replaces the worker's MCP server section for the
  selected executor. Deployment-local entries would otherwise make remote
  behavior differ from the coordinator and can retain stale credentials.
- The serialized snapshot is limited to 1 MiB. MCP definitions are normally a
  few kilobytes; this leaves ample room while remaining far below the signed
  request body's broader transport ceiling.
