# Clarifications: pinned Slack MCP connector from the maintained fork

Each answer is backed by an observation made in this environment, not by
assumption.

## C1 — Delivery mechanism

**Q**: With no npm publish rights for a fork-controlled package name, is a
GitHub release asset consumed via `npx` acceptable, versus a Go-toolchain or
container-based launch?

**A**: GitHub release asset consumed via `npx`. Evidence:

- `npm whoami` → `ENEEDAUTH`, and `slack-mcp-server` on npm is owned by
  `korotovsky` (`npm view slack-mcp-server repository` → upstream). No
  fork-scoped registry credentials exist, so "publish a fork npm version"
  cannot be executed or verified in this environment. It stays the documented
  future option.
- The fork's `go.mod` still declares `module github.com/korotovsky/slack-mcp-server`,
  so `go run github.com/davidvasandani/slack-mcp-server/...@<rev>` cannot
  resolve without renaming the module and every import — a large, upstream-merge-
  hostile diff — and it would add a Go toolchain prerequisite for users.
- A container launch (`docker run …@sha256:…`) is pinnable but adds Docker as a
  hard prerequisite where `npx` is already required by the current entry.
- `npx` accepts a remote tarball URL as a package spec; verified here by running
  `npx -y https://registry.npmjs.org/cowsay/-/cowsay-1.6.0.tgz "…"`
  successfully. A GitHub release asset URL is therefore a working install
  source with no registry involvement and no change to `command: "npx"`.

The release ships the six platform binaries plus a small npm-format **launcher**
tarball (never published to a registry) that selects, caches, verifies and execs
the right binary.

## C2 — Fork version identifier scheme

**Q**: How are fork builds distinguished from upstream versions?

**A**: `v<upstream-base-version>-vk.<n>`, cut from a commit whose history
contains `04633fb892dc6dd38c3faffe29ff9b30829560c6`. The upstream base is the
version the fork is rebased on (`1.3.0` today, matching npm `latest`);
`-vk.<n>` counts fork-side releases against that base. It sorts predictably, is
unambiguous in a URL and in the binary's stamped version (which surfaces as
`serverInfo.version` in the MCP `initialize` response — the binary has no
`--version` flag), and can never collide with an upstream tag. A correction is
always a new `-vk.<n+1>` tag; assets under an existing tag are never re-uploaded.

That rule was exercised immediately: `v1.3.0-vk.1` was published first, an
independent review found its build script stamped a timezone-dependent
`BuildTime`, and the correction shipped as **`v1.3.0-vk.2`** — which is the
release VK pins. `v1.3.0-vk.1` remains published and untouched, annotated as
superseded.

## C3 — What is digest-pinned, and where is the digest recorded?

**Q**: The outer install artifact, the platform binary, or both?

**A**: Both, at two layers:

1. **Platform binary** — the launcher embeds a per-platform SHA-256 table
   (`checksums.json`, generated at release build time) and refuses to execute a
   downloaded binary whose digest does not match. This is the enforcing check;
   it runs on every user's machine on first use.
2. **Outer tarball** — its SHA-256 is recorded in this repository next to the
   pinned URL (a constant in `crates/executors/src/mcp_config.rs`) and asserted
   by a `#[ignore]`d network test, mirroring the `cli_tools` convention for
   vendor artifacts. This is the auditing check: it detects a release asset
   replaced under an existing tag.

Renovate's custom manager reads the tag out of `default_mcp.json`; its
`packageRule` blocks auto-merge and states that the digest constant and the docs
version must be refreshed in the same PR.

## C4 — Offline hosts / unsupported platforms

**Q**: Is a local-binary escape hatch in scope?

**A**: Yes. `SLACK_MCP_SERVER_VK_BINARY=/path/to/binary` makes the launcher exec
that binary and skip download and digest checks entirely (the operator has
supplied the build, so the operator owns its provenance). Everything else fails
loudly: an unsupported `platform-arch`, a failed download, or a digest mismatch
writes one diagnostic line to stderr and exits non-zero. There is no fallback to
a different build, and in particular no fallback to upstream npm — silently
running unpinned upstream code is the defect this feature removes.

## C5 — Is the acceptance attachment readable by the test identity?

**Q**: Should end-to-end retrieval of `F0BJX4Y3N5A` return content, or a
permission error?

**A**: Content. `files.info` for `F0BJX4Y3N5A`, called with the connected
identity's `SLACK_MCP_XOXP_TOKEN`, returns:

| field | value |
| --- | --- |
| `name` | `Re: TAM Introduction - sweetgreen` |
| `filetype` / `mimetype` | `email` / `text/html` |
| `size` | 240,818 bytes (under the connector's 5 MB cap) |
| `groups` | `["C0BE62MCDU6"]`, `is_public: false` |

So the file is a private-channel email attachment the connected identity can
read, and the acceptance run should return metadata plus content. Slack still
enforces the boundary — the file is not public, and identities outside
`C0BE62MCDU6` get a Slack-origin error rather than content.

## Incidental finding (folded into the spec)

`AttachmentIDs` already appears in message metadata from **upstream** 1.3.0 —
the currently connected server returns e.g.
`F0BE9LRCQMQ (Test Cases: Legion PTO -> GCal for Coaches)` in that column. The
fork's contribution is the retrieval tool `attachment_get_data` (plus richer
attachment metadata), not the IDs themselves. FR-6 is therefore a
*preserved-behaviour* requirement, not new functionality.

## Still open (non-blocking)

- **Registry publishing**: if fork-scoped npm ownership is obtained later, the
  launcher can be published under that scope and the catalog can move to a
  `name@version` spec. Not blocking; the release-asset URL is already pinned and
  digest-verified.
- **macOS Gatekeeper**: cross-compiled binaries are unsigned. Whether a
  downloaded `darwin-arm64` binary is quarantined in practice cannot be tested
  from this Linux host; the launcher must surface such an exec failure verbatim,
  and the docs note the escape hatch as the workaround if it reproduces.
