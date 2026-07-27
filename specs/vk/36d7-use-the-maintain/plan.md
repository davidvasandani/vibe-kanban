# Implementation Plan: pinned Slack MCP connector from the maintained fork

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

- **This repo (Rust)**: the change surface is `crates/executors` —
  `default_mcp.json` (the canonical catalog, `include_str!`-embedded by
  `mcp_config.rs:31`) and the `#[cfg(test)]` module at the bottom of
  `crates/executors/src/mcp_config.rs` (Slack tests at lines ~616–662).
  `executors` already depends on `sha2 = "0.10"` and workspace `reqwest`
  (`crates/executors/Cargo.toml`), so the digest test adds **no** new
  dependency — which matters, because the constitution requires recording any
  new top-level dependency.
- **Artifact repo (Go)**: `davidvasandani/slack-mcp-server`, module path still
  `github.com/korotovsky/slack-mcp-server`, default branch `master`, HEAD
  `04633fb` (the attachment merge). `make build-all-platforms` cross-compiles
  `./build/slack-mcp-server-<os>-<arch>[.exe]` for
  `{darwin,linux,windows} × {amd64,arm64}` and stamps
  `git describe --tags` into `pkg/version.Version`. There is **no `--version`
  flag** (`cmd/slack-mcp-server/main.go` registers only `-t/--transport`,
  `-e/--enabled-tools`, `--no-cache`); the stamped value surfaces through
  `server.NewMCPServer("Slack MCP Server", version.Version, …)` in
  `pkg/server/server.go`, i.e. as `serverInfo.version` in the MCP `initialize`
  response. Local toolchain: Go 1.26.3, Node 24.15, npm 11.12.
- **Launch model**: VK is an MCP config *writer* (see
  `docs/knowledge-base/mcp-connectivity-testing.md`); the catalog value is a
  command line handed to each agent. So the artifact must be self-installing
  from a plain command — no VK-side install hook exists for MCP servers.

## Architecture & Approach

### 1. The artifact (fork repo)

`packaging/npm-launcher/` is added to the fork: a dependency-free npm package
whose single `bin` resolves `${process.platform}-${process.arch}` to a release
asset, caches it under `<cache-root>/slack-mcp-server-vk/<version>/`, verifies
SHA-256 against an embedded `checksums.json`, then spawns it with
`stdio: 'inherit'` and mirrors exit code / signal. `scripts/build-release.sh`
drives `make build-all-platforms`, generates `checksums.json`, `npm pack`s the
launcher, and writes `checksums.txt`. Tag `v1.3.0-vk.2` → GitHub release with
six binaries + launcher tarball + checksums.

Mapping to requirements: FR-1/FR-2 (tag cut from a commit whose history contains
`04633fb`), FR-8 (digest enforcement, loud failure), FR-8a
(`SLACK_MCP_SERVER_VK_BINARY`), FR-5/FR-6/FR-7 (inherited from the fork's Go
code — `shouldAddTool(ToolAttachmentGetData, enabledTools, "")` in
`pkg/server/server.go` and the Slack-token calls in `pkg/handler/`).

### 2. The catalog entry (`crates/executors/default_mcp.json`)

Only `slack.args[1]` changes: `slack-mcp-server@latest` →
`https://github.com/davidvasandani/slack-mcp-server/releases/download/v1.3.0-vk.2/slack-mcp-server-vk-1.3.0-vk.2.tgz`.
`command`, `--transport stdio`, `env`, `meta.slack.*`, and every other server
stay byte-identical (FR-3, FR-4, FR-12). The entry remains transport-neutral
with a credential placeholder, so `apply_adapter` keeps producing Codex `env`
and Opencode `environment` unchanged — the constraint recorded in
`docs/knowledge-base/shared-mcp-configuration.md`.

### 3. The pin's guardrails (`crates/executors/src/mcp_config.rs`)

Two module-level constants next to the tests, so one place defines the pin for
assertions and for humans:

```rust
/// Pinned fork release tag; see docs/integrations/mcp-server-configuration.mdx.
const SLACK_MCP_FORK_TAG: &str = "v1.3.0-vk.2";
/// SHA-256 of the pinned launcher tarball asset (audited by the ignored test).
const SLACK_MCP_LAUNCHER_SHA256: &str = "<recorded at release time>";
```

Tests:

1. `slack_preconfigured_server_matches_the_documented_stdio_contract` — updated
   to the exact pinned args (FR-4).
2. `slack_preconfigured_server_adapts_for_codex_and_opencode` — updated; still
   asserts Codex `env` / Opencode `environment` (FR-4).
3. `slack_preconfigured_server_pins_an_immutable_fork_artifact` (new) — parses
   the spec, requires the
   `https://github.com/{owner}/{repo}/releases/download/{tag}/{asset}` shape,
   asserts `{owner}/{repo}` equals the owner/repo parsed out of
   `meta.slack.url`, asserts `{tag} == SLACK_MCP_FORK_TAG`, and rejects
   `@latest`, `#master`, `refs/heads/`, `/archive/` (FR-2, FR-3).
4. `slack_pinned_launcher_matches_recorded_digest` (new, `#[ignore]`) —
   `reqwest::get` the pinned URL, `sha2::Sha256` the bytes, compare with
   `SLACK_MCP_LAUNCHER_SHA256` (FR-9). Mirrors the `cli_tools` "deliberate
   network test" convention.

### 4. Update process (`renovate.json`, docs)

A third `customManagers` entry matches the tag inside `default_mcp.json`:

```json
{
  "customType": "regex",
  "description": "Slack MCP fork release pin in default_mcp.json",
  "managerFilePatterns": ["/crates/executors/default_mcp\\.json$/"],
  "matchStrings": [
    "https://github\\.com/(?<depName>davidvasandani/slack-mcp-server)/releases/download/(?<currentValue>v[^/\"]+)/"
  ],
  "datasourceTemplate": "github-releases"
}
```

plus a `packageRules` entry for that depName with `automerge: false`,
`addLabels: ["needs-review"]`, **`ignoreUnstable: false`** (fork tags are semver
prereleases — without this Renovate matches the pin and then silently never
proposes an update), an explicit semver `versioning`, and a `prBodyNotes` line
naming the three things that must move together (URL,
`SLACK_MCP_LAUNCHER_SHA256`, docs). This mirrors the existing CLI-tool-catalog
rule, which exists for exactly this reason (FR-10).

Docs (FR-11): a "Slack" section in
`docs/integrations/mcp-server-configuration.mdx` naming the installed fork
version and revision, `attachment_get_data` default-on plus
`SLACK_MCP_ENABLED_TOOLS` exclusion, the `SLACK_MCP_SERVER_VK_BINARY` escape
hatch, and a release-cutting checklist. One line in `CLAUDE.md`'s Dependencies
section marks the pin as Renovate-managed and not hand-bumpable. The Slack
*shortcut* page (`docs/integrations/slack-integration.mdx`) is untouched — it
documents `crates/remote`, a different subsystem.

## Data Model

No persisted entities. The only structured data is the launcher's
`checksums.json` (`{version, tag, assets: [{platform, arch, file, sha256, size}]}`) —
described in `./data-model.md`.

## Contracts

`./contracts.md` records the two interfaces this change fixes in place: the
catalog entry JSON shape (and its per-agent adaptations) and the launcher's CLI
/ environment / exit-status contract.

## Research Notes

`./research.md` — delivery-mechanism comparison (npm publish, Go module,
container, git spec, release asset), the `npx` tarball-URL verification, why the
launcher downloads at run time rather than in `postinstall`, and why no new
Rust dependency is needed.

## Constitution Check

| Principle | How this plan honours it |
| --- | --- |
| I. Clarity over cleverness | One JSON string changes; the pin's meaning is stated in two named constants and in docs, not inferred. |
| II. Test the contract | Acceptance criteria precede implementation; the pin shape, the adapters, and the published digest each get a test. |
| III. Small, reversible steps | No new VK subsystem, no new dependency, no frontend work; reverting is a one-line JSON edit. |
| VI. Don't rebuild what shipped | Reuses the `cli_tools` pinning idiom (immutable URL + SHA-256 + Renovate + ignored network test) and `mcp_test.rs` for verification instead of new machinery. |
| VIII. Managed tools are pinned, verified, user-owned | Version-addressed URL, exact SHA-256, staged download + atomic rename, loud failure, credentials stay in the user's env. |
| XV. Bundled entries install what they advertise | The install URL and `meta.slack.url` name the same repository, by construction and by test; `@latest` is banned by test. |
| Constraint: transport-neutral catalog | `command`/`args`/`env` with a placeholder; adapters untouched. |
| Constraint: no new top-level deps | `sha2` and `reqwest` are already `executors` dependencies. |
| Constraint: `pnpm run format` | Runs in the verification phase. |

No deviations.

## Risks & Dependencies

- **Fork release publishing** depends on GitHub admin rights on
  `davidvasandani/slack-mcp-server` (confirmed: `permissions.admin: true`) and
  is an outward-facing, user-visible action — it creates a public tag and
  release. Mitigation: build locally and publish with `gh release create` rather
  than pushing a tag that triggers the fork's `release.yaml`, whose final step
  (`make npm-publish`) targets the **upstream** npm package name.
- **Release-asset mutability**: GitHub permits replacing assets under a tag.
  Mitigated by the recorded digest + ignored network test + "new tag per
  correction" rule.
- **Unsigned macOS binaries** may be quarantined; unverifiable from this Linux
  host. Mitigated by loud launcher errors and the documented escape hatch.
- **npm behaviour for URL specs** verified on npm 11.12 only; documented as the
  assumed floor (npm 7+).
- **Real-Slack acceptance** depends on the connected `SLACK_MCP_XOXP_TOKEN`
  identity retaining access to `C0BE62MCDU6` / `F0BJX4Y3N5A`.
