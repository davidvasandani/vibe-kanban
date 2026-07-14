# Technical Specification: Atlassian CLI in the Managed CLI Catalog

## Objective

Add Atlassian CLI (`acli`) to Vibe Kanban's managed CLI tools so users can
discover, install, update, remove, and expose a verified app-owned copy to
spawned agents through the existing CLI Tools settings workflow.

## Background

Vibe Kanban maintains a curated catalog in
`crates/services/src/services/cli_tools.rs`. Each catalog entry has a stable
wire identifier, pinned version, platform-specific source and SHA-256 digest,
installation strategy, version probe, display metadata, and vendor docs link.
The existing service and UI derive their behavior from this catalog.

Atlassian publishes standalone ACLI artifacts for Linux amd64 and arm64. The
vendor's Linux guide documents both direct binaries and tar archives. The
managed installer must use a version-stable, checksum-pinned source rather
than an unverified moving `latest` artifact.

## Functional Requirements

1. Add `acli` as a serializable `CliToolId` whose wire and directory name is
   `acli`, and include it in the complete catalog ordering.
2. Present the tool as "Atlassian CLI" with concise Atlassian Cloud command
   line metadata and link users to Atlassian's official ACLI documentation.
3. Pin a concrete ACLI release and provide SHA-256 digests for every supported
   platform artifact.
4. Support vendor-published Linux x86-64 and Linux arm64 builds. Other hosts
   must report a clear unsupported reason through existing behavior.
5. Install the executable as `acli` using the existing atomic staged archive
   workflow, and probe host copies with `acli --version`.
6. Preserve all existing host-copy precedence, credential ownership,
   update/outdated reporting, removal, and agent PATH behavior.
7. Ensure generated shared TypeScript types include the new `acli` identifier
   through the repository's normal type-generation workflow; generated files
   must not be hand-edited.

## Integrity and Safety Requirements

- Every downloadable artifact must be authenticated by an exact SHA-256 pin
  before extraction or promotion.
- A failed download, checksum, or extraction must leave no partial executable
  on agents' PATH.
- ACLI authentication and configuration remain user/host managed; Vibe Kanban
  does not collect or persist Atlassian credentials.
- Existing managed tools and their wire identifiers remain backward
  compatible.

## Verification

- Unit tests assert ACLI's catalog identity, supported platform mappings,
  archive/binary layout, and docs/version probe metadata.
- Existing CLI-tool service tests continue to pass.
- Rust formatting and targeted service tests/checks pass.
- Shared types are regenerated or verified with the repository generator.
- An independent Codex diff review reports no significant findings.

## Out of Scope

- Managing Atlassian authentication, sites, tokens, or ACLI configuration.
- Adding ACLI as a coding-agent executor.
- Installing through system package managers or requiring root privileges.
- Supporting platforms for which no verified vendor artifact is selected.
