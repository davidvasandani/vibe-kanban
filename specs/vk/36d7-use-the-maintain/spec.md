# Feature Specification: pinned Slack MCP connector from the maintained fork

**Feature dir**: `specs/vk/36d7-use-the-maintain/`
**Status**: Draft

## Summary

Vibe Kanban's bundled Slack MCP catalog entry advertises
`https://github.com/davidvasandani/slack-mcp-server/` as its source but installs
`slack-mcp-server@latest` from npm, which is the upstream `korotovsky` package
(currently 1.3.0). Users therefore get neither the fork's attachment-retrieval
tool (`attachment_get_data`, merged in fork commit
`04633fb892dc6dd38c3faffe29ff9b30829560c6`) nor a reproducible install — the
`@latest` tag can change under them at any time. This feature repoints the
catalog entry at a pinned, digest-verified artifact built from the fork, keeps
the advertised source URL and the installed code the same repository, exposes
`attachment_get_data` by default, and establishes a written process for adopting
future fork revisions.

## User Stories

- As a coding-agent user, I want the Slack connector VK configures to be able to
  read a message's attachments, so an agent can act on files shared in Slack
  instead of stopping at "no file tool exists".
- As a coding-agent user, I want the connector VK installs today to be the same
  code it installs next month, so a working setup does not break because an
  upstream tag moved.
- As a security-minded operator, I want the artifact VK tells agents to run to
  be pinned to a version and verifiable against a recorded digest, so a
  compromised or silently republished dependency is detectable.
- As a maintainer, I want the catalog's source link and its install source to be
  the same repository, so the UI does not misrepresent what runs on the user's
  machine.
- As a maintainer, I want a documented, dependency-bot-assisted path to the next
  fork revision, so the pin does not rot or get hand-bumped incorrectly.

## Functional Requirements

- **FR-1**: The bundled Slack catalog entry MUST install a build produced from
  the `davidvasandani/slack-mcp-server` fork whose history contains
  `04633fb892dc6dd38c3faffe29ff9b30829560c6` (or a newer reviewed fork
  revision).
- **FR-2**: The install specification MUST identify one immutable revision — a
  released version/tag or digest. `@latest`, a bare branch, and any other
  mutable reference are prohibited.
- **FR-3**: The repository named in the install specification MUST match the
  repository in the entry's catalog metadata URL.
- **FR-4**: The entry MUST keep its current launch contract: a stdio transport
  ("--transport stdio") and the `SLACK_MCP_XOXP_TOKEN` credential placeholder,
  adapted per coding agent exactly as today (including Opencode's
  `environment` field).
- **FR-5**: A newly configured connector MUST expose `attachment_get_data` in
  its tool list with no extra configuration; it MUST be absent only when the
  user's `SLACK_MCP_ENABLED_TOOLS` selection excludes it.
- **FR-6**: Message metadata for a message that carries files MUST surface a
  stable attachment identifier usable as `attachment_get_data` input. (This
  behaviour already exists upstream — the requirement is that the re-pin
  preserves it.)
- **FR-7**: `attachment_get_data` MUST perform retrieval as the connected Slack
  identity. Channel and file authorization stays with Slack; the feature adds no
  bypass, and permission/scope failures surface as actionable errors.
- **FR-8**: The delivered artifact MUST be verified against a recorded
  cryptographic digest before execution, and MUST fail loudly — no silent
  fallback to a different build, and never a fallback to unpinned upstream code
  — when verification, download, or platform support fails.
- **FR-8a**: An operator-supplied local build MUST be usable in place of the
  downloaded artifact (offline, air-gapped, or unsupported-platform hosts), via
  an explicit opt-in that makes the operator the provenance owner.
- **FR-9**: The recorded pin (version/tag and digest) MUST be asserted by
  automated tests in this repository, including a deliberately-run test that
  checks the published artifact against the recorded digest.
- **FR-10**: The repository's dependency-update automation MUST track the fork
  release used by the entry, and MUST NOT auto-merge such an update: the PR is
  labelled for human review and states that the digest and documentation must be
  refreshed in the same change.
- **FR-11**: Documentation MUST state the exact fork version/revision installed,
  that `attachment_get_data` ships by default, how to exclude it, and the
  step-by-step process for cutting and adopting the next fork release.
- **FR-12**: No other catalog entry may change.

## Out of Scope

- Publishing the fork to any npm registry (no fork-scoped registry credentials
  exist; recorded as the future option).
- The `crates/remote` Slack **app** integration (Create issue from message
  shortcut) — a different subsystem that shares only the word "Slack".
- Rendering catalog suggestions in the shared MCP settings UI.
- Upstreaming the attachment work to `korotovsky/slack-mcp-server`.
- Changing which tools other than `attachment_get_data` the connector exposes.

## Acceptance Criteria

- [ ] A newly configured VK Slack MCP server installs/runs code built from the
      David Vasandani fork (verified in a clean cache, so no prior `npx` cache
      can mask the artifact), and the resolved artifact version is recorded.
- [ ] The install specification names one immutable revision; searching the
      catalog file for `@latest`/branch references in the Slack entry returns
      nothing.
- [ ] `tools/list` on a freshly launched connector includes
      `attachment_get_data`; with `SLACK_MCP_ENABLED_TOOLS` set to a list that
      excludes it, it is absent.
- [ ] Searching `https://sweetgreen.slack.com/archives/C0BE62MCDU6/p1784648794618929`
      through the connector returns attachment ID `F0BJX4Y3N5A`.
- [ ] Calling `attachment_get_data` for `F0BJX4Y3N5A` with the connected Slack
      identity reaches the retrieval handler and returns the file's metadata and
      content — never "unknown tool". (Per C5 the file is a 240 KB private-channel
      email the connected identity can read, so content is the expected result.)
- [ ] A file the connected identity cannot access still fails with a
      Slack-origin authorization error, not with content.
- [ ] `cargo test -p executors` passes, including the new pin-shape test; the
      digest test passes when deliberately run with network access.
- [ ] Repository checks pass: `pnpm run check`, `pnpm run lint`,
      `pnpm run format`.
- [ ] Documentation names the fork version/revision in use, and the update
      process is written down.
- [ ] `git diff` on the catalog file touches only the Slack entry.

## Resolved Clarifications

Answers and their evidence live in [`clarifications.md`](clarifications.md):

- **C1 — delivery**: a GitHub release asset from the fork, installed via
  `npx <release-tarball-url>` (npm publishing is unavailable; Go-module and
  container launches were rejected with reasons).
- **C2 — versioning**: `v<upstream-base>-vk.<n>`, first release `v1.3.0-vk.2`;
  corrections get a new `-vk.<n+1>` tag, assets are never re-uploaded.
- **C3 — digests**: two layers — per-platform binary digests enforced by the
  launcher at run time, and the outer tarball digest recorded in this repo and
  audited by a deliberately-run network test.
- **C4 — escape hatch**: `SLACK_MCP_SERVER_VK_BINARY` runs an operator-supplied
  build; every other failure path exits non-zero with a diagnostic.
- **C5 — acceptance attachment**: `F0BJX4Y3N5A` is a 240 KB private-channel
  email readable by the connected identity, so end-to-end retrieval must return
  content.

## Open Questions

None blocking. Two items tracked for later, both recorded in
`clarifications.md`: moving to a registry-published launcher if fork-scoped npm
ownership is obtained, and confirming macOS Gatekeeper behaviour for the
unsigned `darwin-*` binaries on a real macOS host.
