# Close the unverified-install gap in the pinned Slack MCP connector

Task: `95e9-close-the-unveri`

## Problem

The predecessor task (`36d7-use-the-maintain`) pins the bundled Slack MCP
connector to a launcher tarball hosted as a GitHub release asset. The launcher
verifies the selected platform binary before executing it, but `npx` downloads
and executes the outer tarball without accepting an expected integrity value.
Because a GitHub release asset can be replaced under an existing tag, control
of the fork's releases would let an attacker replace both the launcher and its
baked-in binary digest table. The weekly recorded-digest audit detects that
replacement after the fact; it does not prevent first execution.

This is inherited risk, not a regression from the former mutable
`slack-mcp-server@latest` entry.

## Goal

Ensure that the bundled Slack connector is delivered through a mechanism that
authenticates or verifies the outer package before any fork-controlled code
executes. If no enforceable mechanism can be shipped without unavailable
external ownership or credentials, record an explicit, reviewable decision to
retain detection-only controls, including the condition that reopens that
decision.

## Dependencies and baseline

This task builds on commit `2e4b77aa` / task `36d7-use-the-maintain`, which adds:

- the fork-controlled `v1.3.0-vk.2` launcher release;
- its pinned URL in `crates/executors/default_mcp.json`;
- per-platform binary verification inside the launcher;
- `SLACK_MCP_LAUNCHER_SHA256` and an ignored network audit test;
- the scheduled pinned-artifact audit;
- documentation and Renovate coverage for the GitHub release pin.

The baseline must be incorporated before implementation if it is not already
an ancestor of the working branch.

## Technical approach to evaluate

### Preferred: immutable npm registry package

Publish the launcher under a fork-controlled npm package name, then change the
catalog entry to `npx -y <name>@<exact-version> --transport stdio`. npm's
packument supplies `dist.integrity`, and registry tarballs cannot be replaced
at an existing name/version. npm verifies the tarball before its `bin` runs.

This is the smallest repository-side design, but it is deliverable only if the
maintainer controls the package namespace and an npm publish credential is
available. The implementation must not fabricate either prerequisite.

### Enforceable fallback: VK-managed installation

If npm publication is unavailable, extend the managed CLI tool catalog so VK
downloads the version-addressed launcher or platform executable, verifies its
recorded SHA-256 before installation, stages it atomically, and exposes a
stable executable path usable by generated MCP configuration. This closes the
outer-artifact gap but requires an explicit per-user installation lifecycle and
must define behavior when the executable is absent.

### Complementary signing

Signature verification may strengthen either delivery mechanism but cannot, by
itself, protect an outer launcher that executes before the signature check.

### Detect-only exception

Retaining the current GitHub URL package is acceptable only as a written
decision when both preventative delivery paths are presently blocked or impose
cost disproportionate to the threat. The record must state:

- which prerequisite is missing;
- why the managed installer is not being adopted;
- the remaining attack and detection window;
- the owner/process for audit failure notification; and
- the concrete trigger for revisiting prevention.

Daily auditing may reduce exposure but must not be described as prevention.

## Functional requirements

1. The bundled Slack MCP entry continues to use stdio and preserves
   `SLACK_MCP_XOXP_TOKEN`.
2. A clean first launch must not execute fork-controlled bytes until the outer
   delivery artifact has passed an integrity/authenticity check, unless the
   detect-only exception is explicitly chosen and documented.
3. The launcher continues to verify the platform binary before execution.
4. The catalog continues to adapt correctly for Codex and Opencode.
5. The fork repository link remains visible in catalog metadata.
6. An end-to-end clean-cache probe must initialize the connector, observe
   `attachment_get_data` in `tools/list`, and retrieve a real attachment when
   suitable Slack credentials and fixture data are available.

## Repository consistency requirements

If delivery changes:

- update `crates/executors/default_mcp.json`;
- update the immutable-pin tests in
  `crates/executors/src/mcp_config.rs`;
- update or remove the launcher digest constant and its network audit so they
  describe the new outer artifact accurately;
- update `docs/integrations/mcp-server-configuration.mdx`;
- update the Renovate custom manager and package rule to track the new source;
- update `docs/knowledge-base/forked-mcp-server-packaging.md`.

All version, artifact, digest, documentation, and Renovate changes must land
together.

## Verification

- Run focused executor tests for the Slack catalog contract and adapter shapes.
- Run the ignored published-artifact integrity test when it remains applicable.
- Validate `renovate-config-validator renovate.json` (or the repository's
  equivalent).
- Run repository formatting and proportionate checks after dependency setup.
- Exercise the connector with an empty npm cache and absent launcher cache.
  Credential-dependent attachment retrieval may be reported as externally
  blocked only after the unauthenticated handshake and tool-registration checks
  have passed and the exact missing credential/fixture is identified.

## Non-goals

- Reworking the Slack MCP server's tool behavior.
- Treating signatures inside an unverified launcher as sufficient protection.
- Claiming a shorter audit interval eliminates the release-writer threat.
- Publishing to an npm namespace without explicit ownership and credentials.

## Acceptance

The task is complete when either:

1. the bundled connector's outer artifact is verified before execution and all
   catalog, test, documentation, Renovate, and end-to-end requirements remain
   satisfied; or
2. a committed decision record accepts detection-only risk with the rationale,
   controls, notification path, and reopening trigger specified above.
