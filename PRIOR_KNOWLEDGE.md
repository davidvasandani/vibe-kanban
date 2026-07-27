# Prior knowledge for the verified Slack MCP delivery task

Task: `95e9-close-the-unveri`

The project knowledge base is not empty. The following pages and established
patterns are relevant.

## Forked MCP server packaging

Source:
`docs/knowledge-base/forked-mcp-server-packaging.md` from predecessor task
`36d7-use-the-maintain` (available in commit `2e4b77aa`, which this branch has
not yet incorporated).

- The catalog URL must identify the fork that actually supplies the executable;
  metadata alone does not select a build.
- The fork launcher verifies a baked-in digest for each platform binary before
  execution and uses staged download plus atomic promotion.
- The operator override intentionally transfers provenance responsibility to
  the operator.
- The launcher tarball digest and platform binary digests answer different
  questions. The inner digest is enforced on every clean launch. The outer
  digest is only audited because `npx` cannot accept integrity for a URL package.
- GitHub release assets can be replaced under an existing tag. A scheduled
  digest test makes replacement detectable, not impossible.
- Version corrections use a new `-vk.<n+1>` tag; existing assets are never
  re-uploaded.
- Renovate must move both occurrences of the version in the GitHub URL,
  include prerelease fork tags, disable automerge, and remind reviewers to
  refresh the source constant, digest, and documentation together.
- Real verification isolates both npm's cache and the launcher's cache and
  drives the stdio handshake through `tools/list`.

Implication: this task must protect the outer artifact before the package's
`bin` runs. Adding a signature check inside that same unverified launcher does
not close the gap.

## MCP connectivity testing

Source: `docs/knowledge-base/mcp-connectivity-testing.md`.

- VK primarily writes MCP definitions into agents' native configuration; it
  does not own the agent's normal MCP install or launch lifecycle.
- The on-demand probe is the limited exception: it spawns the exact configured
  stdio command and performs `initialize`, `notifications/initialized`, and
  `tools/list`.
- Stdout is protocol-only for stdio servers; diagnostics belong on stderr.
- Spawned processes must be bounded and killed on timeout.
- A handler argument error can prove a tool exists and ran even when a
  successful credential-backed call is unavailable.

Implication: pointing a preconfigured entry at an app-managed executable
requires a new lifecycle contract. A catalog string cannot silently assume that
the per-user executable was installed.

## Shared MCP configuration

Source: `docs/knowledge-base/shared-mcp-configuration.md`.

- `crates/executors/default_mcp.json` is the canonical suggested-server
  catalog.
- Canonical stdio entries use `command`, `args`, and `env`; adapters in
  `mcp_config.rs` translate them to each agent's native shape.
- Opencode needs an array command and renames `env` to `environment`.
- Catalog availability and UI discoverability are separate concerns.

Implication: preserve the canonical Slack entry's token placeholder and add
tests for the unadapted, Codex, and Opencode forms after any delivery change.

## App-managed CLI tool installer

Source: `crates/services/src/services/cli_tools.rs`, with the module-specific
precedent documented in
`docs/knowledge-base/powershell-module-cli-tools.md`.

- Managed tools live below `assets::cli_tools_dir()`, with only a stable `bin`
  directory exposed to spawned agents.
- Archive installs download to per-tool staging, stream-compute SHA-256, compare
  against a per-platform pin before extraction, promote a complete version
  directory, and swap the executable symlink last.
- A checksum mismatch writes no install.
- Host copies win over app-owned copies for normal CLI tools.
- Installation is explicit and per-user; status, install, removal, platform
  support, and manifest state are all modelled.
- The current catalog assumes one exposed executable per tool and is surfaced
  as a user-facing vendor CLI catalog, not as an implementation detail for
  preconfigured MCP entries.

Implication: this machinery can enforce outer integrity, but adopting it is not
just adding a checksum row. The plan must account for installation UX,
generated-config path stability, absence/outdated behavior, platform assets,
and whether a host binary is allowed to supersede the fork pin.

## Worktree verification prerequisites

Source: `docs/knowledge-base/worktree-formatting-prerequisites.md`.

- Run `pnpm install --frozen-lockfile` before repository formatting and broad
  verification in a fresh worktree.
- The formatter preflight intentionally fails before mutation when package-local
  formatter shims are absent.

Implication: dependency setup is an explicit plan task before final formatting.

## Decision constraints distilled from prior knowledge

1. npm publication is the least invasive prevention mechanism, but package
   ownership and publish credentials are external prerequisites and must be
   verified rather than assumed.
2. The managed installer is technically capable of verification before
   execution, but it changes the MCP catalog's installation contract and must
   not create a broken default entry for users who have not installed the tool.
3. Signature verification is complementary only after the outer fetch itself is
   authenticated or digest-verified.
4. If prevention cannot be shipped responsibly in this repository state, an
   honest detect-only decision must preserve the scheduled audit and state the
   exact condition that reopens prevention.
