# Research: verified Slack MCP installation

## Decision

Ship an explicit temporary detect-only exception, tighten the existing audit to
daily, and create/update a GitHub issue when it fails.

## Evidence

### npm delivery is not currently actionable

On 2026-07-27:

- `npm whoami` returned `ENEEDAUTH`;
- querying `@davidvasandani/slack-mcp-server-vk` returned `E404`.

An exact npm package would be the preferred solution because registry metadata
provides integrity before a package's executable runs, but this task has neither
a published package nor authority to publish one.

### Managed installation is technically sound but product-expansive

`crates/services/src/services/cli_tools.rs` already provides:

- version-addressed platform archives;
- streamed SHA-256 verification;
- per-tool staging;
- extraction after verification;
- atomic version promotion and final symlink exposure;
- status/install/remove APIs and UI.

Using it for Slack would nevertheless require a new bridge between suggested
MCP definitions and managed installations. Static catalogue JSON cannot encode
the per-user app-data path, and an entry that names a missing PATH executable
would fail for every clean user until a separate install action is completed.
Normal managed tools also allow a host copy to win, which conflicts with the
requirement to execute the reviewed fork build. Solving these is a distinct
product slice, not a checksum-table edit.

### Signing alone does not protect the outer launcher

A signature verifier placed inside the GitHub tarball begins only after npm has
downloaded and executed that tarball's `bin`. A release writer replacing both
the verifier and its trust material still wins. Signing is complementary only
when a trusted delivery layer or VK-owned installer verifies it first.

### Daily detection plus durable notification is the smallest honest control

The predecessor already records the expected tarball SHA-256 and runs the real
download check on a schedule. Daily cadence reduces expected exposure. A
failure-only GitHub issue creates durable, assignable evidence without another
secret or notification service. Reusing an open issue avoids daily duplicates
during a sustained incident.

## Alternatives rejected

- **Pretend the recorded SHA-256 is install-time verification**: false; npm does
  not consume it.
- **Only change weekly to daily**: insufficiently explicit without a decision
  record and durable failure routing.
- **Publish from this task**: unauthorised and impossible without credentials.
- **Add Slack to the managed CLI enum only**: creates a disconnected install
  button but does not make the static MCP entry reliably resolve that install.

## New dependencies

None. The workflow uses the existing GitHub Actions environment and
`actions/github-script` pinned to a reviewed major/full revision according to
repository convention.

