# Research Notes: pinned Slack MCP connector from the maintained fork

## D1 — How does the fork's code reach a user's machine?

Five mechanisms were considered. All must satisfy: pinned/reproducible, no VK
install hook required (VK only writes command lines), and no new hard
prerequisite beyond today's `npx`.

| Option | Pinned? | Blocker |
| --- | --- | --- |
| Publish a fork npm package | yes | **No credentials.** `npm whoami` → `ENEEDAUTH`; `slack-mcp-server` is owned by `korotovsky`. Cannot be executed or verified here. |
| `go run github.com/davidvasandani/…@<rev>` | yes | Fork's `go.mod` is `module github.com/korotovsky/slack-mcp-server`; the import path doesn't match the fork, so the proxy can't resolve it. A module rename touches every import and fights future upstream merges. Also adds a Go prerequisite. |
| `docker run …@sha256:…` | yes (digest) | Adds Docker as a hard prerequisite where `npx` already suffices; heavier for a stdio MCP server. |
| `npx github:davidvasandani/slack-mcp-server#<sha>` | yes (sha) | Needs a root `package.json` in a Go repo, clones ~21 MB per cache miss, and still needs a binary source at run time. Kept as fallback. |
| **`npx <GitHub release tarball URL>`** | **yes (tag)** | **Chosen.** No registry, no new prerequisite, URL names the fork. |

**Verification that the chosen spec form works** (this environment, npm 11.12):

```
$ npx -y https://registry.npmjs.org/cowsay/-/cowsay-1.6.0.tgz "npx url spec works"
 ____________________
< npx url spec works >
```

npm accepts a remote tarball as a package spec and runs its `bin` with the
trailing argv — exactly the shape `default_mcp.json` needs.

## D2 — Why a launcher package instead of shipping binaries in the tarball?

Upstream's npm layout is a launcher plus six per-platform packages resolved as
`optionalDependencies` from the registry, filtered by each package's `os`/`cpu`
fields. That filtering needs a registry packument; with URL-spec optional
dependencies npm must download every tarball to read its `package.json`, so all
six platforms (~6 × tens of MB) would be fetched on every install. Bundling all
six binaries into one tarball has the same size problem.

A single small launcher that downloads exactly one binary on first use keeps the
install cheap and makes the digest check explicit and testable.

## D3 — Why download at run time, not in `postinstall`?

`postinstall` is skipped entirely under `ignore-scripts=true` (common in
locked-down corporate npm configs) — the bin would then exist but fail with a
missing binary. Resolving on first run works regardless of script policy, keeps
the failure adjacent to the cause, and lets `SLACK_MCP_SERVER_VK_BINARY` bypass
the whole path.

## D4 — Digest strategy

Two layers, because they answer different questions:

- **Enforcement** (does *this machine* run the right bytes?) — per-platform
  SHA-256 baked into the launcher, checked before the binary is moved into the
  cache and before it is executed. Mismatch → non-zero exit with a diagnostic;
  never a fallback build.
- **Audit** (is the *published* artifact still what we pinned?) — the launcher
  tarball's SHA-256 recorded in `mcp_config.rs` and asserted by a `#[ignore]`d
  network test. This is what catches a release asset replaced under an existing
  tag, which GitHub permits.

This mirrors `crates/services/src/services/cli_tools.rs`, whose knowledge-base
page states the rule directly: pin every downloadable artifact with a SHA-256,
prefer immutable version-addressed URLs, refresh hashes in the same change as a
version bump, and keep a deliberately-run network test for vendor artifacts.

## D5 — Dependencies

None added. `crates/executors/Cargo.toml` already lists `sha2 = "0.10"` and
workspace `reqwest`, which is everything the digest test needs. The launcher
package has zero runtime dependencies (Node's `https`, `crypto`, `fs`,
`child_process` only), so it introduces no transitive npm supply chain of its
own.

## D6 — Release automation vs. manual publish

The fork's `.github/workflows/release.yaml` triggers on any tag push, builds all
platforms, uploads a release — and then runs `make npm-publish`, which targets
the **upstream** package name `slack-mcp-server`. That step will fail in the
fork for want of `NPM_TOKEN`, but relying on a failure for safety is poor
practice. Decision: build locally with the fork's own Makefile (so the version
stamp and flags are identical to CI) and publish with `gh release create`, and
note in the fork's release runbook that the npm step must be removed or guarded
before anyone re-enables tag-triggered releases.

## D7 — What the fork actually adds

Checked against the merge commit `04633fb` and the current tree:

- `pkg/server/server.go` registers `ToolAttachmentGetData` (`attachment_get_data`)
  through `shouldAddTool(ToolAttachmentGetData, enabledTools, "")` — no env-var
  gate, so it is on by default and only `SLACK_MCP_ENABLED_TOOLS` removes it.
- `pkg/handler/attachment.go` adds file-ID validation (`^F[A-Z0-9]+$`), a 5 MB
  limited writer, and Slack-error mapping (`missing_scope`, `not_authed`,
  `access_denied`, `file_not_found`, `file_deleted`) with actionable messages —
  i.e. Slack stays the authorization boundary (FR-7).
- `AttachmentIDs` in message metadata already exists **upstream** (observed in
  the currently connected 1.3.0 server's CSV output), so the fork's value here
  is retrieval, not identification.
