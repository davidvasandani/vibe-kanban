# Feature Specification: Add Atlassian CLI to Managed CLI Tools

**Feature dir**: `specs/vk/fc47-atlassian-cli-to/`
**Status**: Clarified (no open questions)
**Task**: `vk/fc47-atlassian-cli-to`

## Summary

Vibe Kanban lets users manage a curated set of command-line tools from inside
the app, then exposes those tools consistently to spawned agents. Atlassian CLI
(`acli`) is not currently available in that managed tools catalog, so users who
want agents or local workflows to interact with Atlassian Cloud must install and
maintain it outside the app.

This feature adds Atlassian CLI as a first-class managed CLI tool. Users should
be able to discover it in the same CLI Tools settings surface as the existing
managed tools, install an app-owned verified copy, update or remove that copy,
and have spawned agents receive the same managed-tool PATH behavior they already
receive for other catalog entries.

## Why

Atlassian CLI is useful for teams whose work in Vibe Kanban connects to Jira,
Confluence, or other Atlassian Cloud resources. Bringing it into the managed
catalog reduces per-user setup friction, makes agent environments more
predictable, and keeps the supply-chain contract centralized: pinned releases,
verified downloads, deterministic install locations, and clear unsupported-host
behavior.

The feature should extend the existing managed CLI experience rather than
creating a separate Atlassian-specific installer. Users should experience
Atlassian CLI as one more trusted tool in Vibe Kanban's managed catalog.

## User Stories

- As a Vibe Kanban user, I want to find "Atlassian CLI" in the managed CLI
  tools list, so I know the app can install and manage it for me.
- As a user who works with Atlassian Cloud, I want Vibe Kanban to install a
  verified `acli` executable, so I can use Atlassian commands without manually
  downloading the tool.
- As a user who runs agents from Vibe Kanban, I want those agents to see the
  managed `acli` executable on PATH, so agent tasks can use the same tool I
  installed through the app.
- As a security-conscious user, I want the installed Atlassian CLI release to be
  pinned and checksum-verified, so failed or tampered downloads are not exposed
  to my workflows.
- As a user on an unsupported host, I want a clear unsupported message instead
  of a broken install attempt.

## Functional Requirements

- FR-1: The managed CLI tools catalog MUST include an Atlassian CLI entry whose
  stable user-facing command name is `acli`.
- FR-2: The tool MUST be presented as "Atlassian CLI" with concise metadata
  explaining that it is Atlassian's command-line tool for Atlassian Cloud
  workflows.
- FR-3: The catalog entry MUST link to Atlassian's official ACLI documentation
  so users can understand authentication, configuration, and command usage.
- FR-4: Users MUST be able to install, update, remove, and inspect Atlassian CLI
  through the same managed CLI Tools workflow used for existing catalog tools.
- FR-5: A managed Atlassian CLI install MUST expose an executable named `acli`
  to spawned agents using the existing managed-tool PATH behavior.
- FR-6: If a compatible host copy of `acli` already exists, existing host-copy
  precedence behavior MUST remain unchanged.
- FR-7: Atlassian CLI version/outdated status MUST be determined through the
  same managed CLI status model used by existing tools.
- FR-8: Unsupported operating system or architecture combinations MUST be
  reported clearly through the existing unsupported-tool behavior.
- FR-9: Adding Atlassian CLI MUST NOT change the behavior, identifiers,
  install state, removal behavior, or PATH behavior of any existing managed CLI
  tool.

## Supply-Chain and Ownership Requirements

- SC-1: Vibe Kanban MUST install only a specific pinned Atlassian CLI release,
  not an unversioned or moving latest artifact.
- SC-2: Every downloadable artifact selected for support MUST have an exact
  SHA-256 checksum before it can be installed.
- SC-3: A failed download, checksum mismatch, extraction failure, or promotion
  failure MUST leave no partial `acli` executable available to spawned agents.
- SC-4: Atlassian authentication, tokens, sites, and local ACLI configuration
  MUST remain user/host owned. Vibe Kanban MUST NOT collect, persist, or manage
  Atlassian credentials as part of this feature.
- SC-5: The feature MUST preserve the managed-tools contract that installs are
  deterministic, removable, and isolated to the app-owned tool location unless
  the user has intentionally provided a host copy.

## Supported Platforms

- SP-1: The feature MUST support the Atlassian-published Linux x86-64 artifact
  for pinned release `1.3.22-stable`:
  `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_amd64.tar.gz`.
- SP-2: The Linux x86-64 artifact MUST be verified with SHA-256 checksum
  `de9e0a60a556e4119428b9072f6ca787e75b9f9a538aa71ebcc8084deb8ca1a6`.
- SP-3: The feature MUST support the Atlassian-published Linux arm64 artifact
  for pinned release `1.3.22-stable`:
  `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_arm64.tar.gz`.
- SP-4: The Linux arm64 artifact MUST be verified with SHA-256 checksum
  `1a9e86d0b46a62a8f1992c1ef98b3af7e9a9ee3f76d0efa215fe1f2d1b2fd139`.
- SP-5: Hosts outside Linux x86-64 and Linux arm64 are out of scope for the
  initial implementation and MUST receive clear unsupported-host behavior.

## Out of Scope

- Managing Atlassian login, tokens, sites, or ACLI configuration.
- Adding Atlassian CLI as a coding-agent executor.
- Adding Jira, Confluence, or Atlassian product workflows beyond making `acli`
  available as a managed command.
- Installing through system package managers or requiring administrator/root
  privileges.
- Supporting platforms without a selected vendor artifact and checksum.
- Hand-editing generated shared TypeScript files.

## Acceptance Criteria

- [ ] "Atlassian CLI" appears in the managed CLI Tools catalog with command name
      `acli`, useful description text, and an official Atlassian documentation
      link.
- [ ] On a supported Linux x86-64 host, installing Atlassian CLI produces a
      usable `acli` executable managed by Vibe Kanban.
- [ ] On a supported Linux arm64 host, installing Atlassian CLI produces a
      usable `acli` executable managed by Vibe Kanban.
- [ ] The selected Atlassian CLI release is pinned and every supported artifact
      is checksum-verified before becoming available.
- [ ] Failed install/update attempts do not leave a partial `acli` executable on
      agents' PATH.
- [ ] Removing the managed Atlassian CLI copy removes app-owned `acli`
      availability without changing a user-managed host copy.
- [ ] Spawned agents receive the same PATH behavior for managed `acli` that
      they receive for existing managed CLI tools.
- [ ] Unsupported hosts receive a clear unsupported response and no attempted
      install.
- [ ] Existing managed CLI tools continue to behave as before.
- [ ] Repository validation covers the catalog entry, supported platform
      mappings, integrity metadata, version/status behavior, and generated type
      exposure.

## Assumptions

- Atlassian continues to publish standalone Linux amd64 and arm64 ACLI
  artifacts suitable for app-managed installation.
- Users are responsible for authenticating and configuring ACLI in their host
  environment before commands that require Atlassian credentials can succeed.
- The existing managed CLI Tools surface, install lifecycle, and agent PATH
  integration are the appropriate user experience for this addition.
- Atlassian's current public docs state that each Atlassian CLI version is
  supported for 6 months after release; future update planning should account
  for that vendor support window, but this spec pins the initial install to
  `1.3.22-stable` for deterministic supply-chain behavior.

## Clarifications (resolved)

- **Pinned release**: Pin `1.3.22-stable` for the initial catalog entry.
  Rationale: Atlassian's documented Linux `latest` artifact downloaded on
  2026-07-14 unpacks to `acli_1.3.22-stable_linux_amd64/acli` and
  `acli_1.3.22-stable_linux_arm64/acli`; running the amd64 binary reports
  `acli version 1.3.22-stable`. Atlassian also serves matching immutable
  versioned Linux tarball URLs under `/linux/1.3.22-stable/`, allowing the
  managed catalog to avoid moving `latest` URLs.
- **Initial platform set**: Support only Linux x86-64 and Linux arm64 in the
  first release. Rationale: the requested scope is Linux installation support,
  Atlassian's official package table lists Linux arm64 and amd64 artifacts, and
  no reviewed evidence requires widening initial support to macOS or Windows.
  macOS and Windows can be considered later as separate platform additions with
  their own pinned URLs, checksums, and validation.
- **Artifact integrity source**: Use the exact SHA-256 checksums recorded in
  SP-2 and SP-4. Rationale: Atlassian exposes downloadable tarballs but no
  adjacent `.sha256` files were available at the checked artifact paths; the
  checksums were computed from the current Atlassian-hosted versioned tarballs
  on 2026-07-14 and match the current `latest` Linux tarballs at that time.
- **Official reference links**: Use Atlassian's ACLI install guide and
  supported package guide as the catalog documentation targets:
  `https://developer.atlassian.com/cloud/acli/guides/install-acli/` and
  `https://developer.atlassian.com/cloud/acli/guides/download-supported-packages/`.

## Open Questions

- None remaining.
