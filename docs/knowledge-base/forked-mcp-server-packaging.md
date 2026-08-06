# Packaging a forked MCP server VK can pin

Contributing tasks: `36d7-use-the-maintain`, `95e9-close-the-unveri`,
`967a-migrate-slack-mc`

How to ship a **fork** of a third-party MCP server through
`crates/executors/default_mcp.json` when the upstream package name is not ours
to publish. Complements [shared-mcp-configuration](shared-mcp-configuration.md)
(the catalog contract itself) and the `cli_tools` pinning idiom.

## The failure this prevents

The catalog's `meta.<server>.url` is a **link**, not a build instruction. A
Slack entry can advertise `github.com/davidvasandani/slack-mcp-server` while
`npx -y slack-mcp-server@latest` installs the upstream package — the UI claims
one repository, the machine runs another, and the fork's tools are simply
absent. Two independent defects hide in one line: wrong source, and a mutable
pin. Assert both, or neither is guaranteed.

## Delivery: GitHub release asset, installed by URL

`npx` accepts a **remote tarball URL** as a package spec
(`npx -y https://…/foo.tgz --flag`), which is the hinge. It means a fork can
ship through the existing `command: "npx"` shape with no npm registry
credentials, no new host prerequisite, and a URL that literally contains the
fork's repository path — so catalog link and executable source agree by
construction.

Alternatives and why they lost:

| Option | Why not |
| --- | --- |
| Publish a fork npm package | Requires registry credentials the project does not have; the upstream name is not ours. |
| `go run github.com/<fork>/…@<rev>` | A fork keeps upstream's `module` path, so the import path never resolves; renaming touches every import and fights future merges. Adds a Go prerequisite. |
| `docker run …@sha256:…` | Pinnable, but adds Docker where `npx` already suffices. |
| `npx github:owner/repo#<sha>` | Pinned, but clones the whole (Go) repo per cache miss and still needs a binary source at run time. |

Publishing per-platform packages as URL-spec `optionalDependencies` does **not**
work: `os`/`cpu` filtering needs a registry packument, so npm downloads all six
platform tarballs. Ship one small launcher that fetches exactly one binary.

## The launcher's non-negotiables

- **Digest before exec.** Per-platform SHA-256 baked in at build time; staged
  download, verify, `chmod`, atomic rename. A mismatch is fatal — never fall
  back to another build, and *especially* never to the unpinned upstream
  package, which is the defect being removed.
- **Resolve at run time, not `postinstall`.** `ignore-scripts=true` is common in
  locked-down npm configs; a `postinstall` download silently never happens and
  the failure surfaces far from its cause.
- **stdout belongs to the transport.** Inherit stdio, put every diagnostic on
  stderr, forward SIGINT/SIGTERM/SIGHUP, exit with the child's code or re-raise
  its signal. A launcher that buffers or annotates stdout breaks MCP framing.
- **An operator escape hatch** (`SLACK_MCP_SERVER_VK_BINARY`) for offline hosts,
  unsupported platforms and local builds; the operator then owns provenance, so
  that path skips the manifest entirely — including when the manifest file does
  not exist.
- **Version-scoped cache** (`…/<pkg>/<version>/<asset>`) so a re-pin never
  collides with an old cache and a rollback re-uses the previous entry.

## Two digests, two questions

- **Enforcement** — the per-platform digest inside the launcher answers "is
  *this machine* running the right bytes?" It runs on every user's first launch.
- **Audit** — the launcher tarball's digest, recorded in `mcp_config.rs` and
  asserted by an `#[ignore]`d network test, answers "is the *published* artifact
  still what we pinned?" GitHub lets a release asset be replaced under an
  existing tag; this is how that is noticed.

The audit layer is genuinely weaker than the enforcement layer, and it is worth
being honest about why: **npm, not VK, fetches the outer tarball**, and `npx`
offers no way to pass an integrity hash for a URL spec. A replaced tarball could
therefore ship a malicious `bin` *and* a matching checksum table, defeating the
per-binary check. VK cannot close that hole — it writes command lines, it does
not install MCP servers. So make the residual risk detectable rather than
pretend it away: a scheduled CI job (`.github/workflows/pinned-artifacts.yml`)
runs the ignored digest test daily. A failure opens or updates a durable GitHub
issue containing the workflow run and new-tag remediation rule. GitHub schedules
are best-effort, so "daily" is the target cadence rather than a guaranteed
24-hour maximum. A digest that nothing re-checks on a schedule is a comment, not
a control.

## Accepted outer-launcher risk (task 95e9)

Task `95e9-close-the-unveri` re-evaluated the outer tarball gap and retained it
as an explicit temporary exception:

- The preferred prevention is a fork-controlled npm package pinned as exact
  `name@version`. npm would verify the packument's `dist.integrity` before the
  package's `bin` runs. The proposed package did not exist and the task
  environment had no npm publication identity (`npm whoami` returned
  `ENEEDAUTH`), so publication was not an authorised repository change.
- VK's managed CLI installer can verify a download before exposing it, but
  using it here is not a catalogue-string substitution. It requires a visible
  per-user install prerequisite, a stable app-data executable-path contract,
  platform lifecycle UI/API wiring, and disabling normal host-copy precedence.
  Adding that product surface while npm publication is blocked only on external
  ownership was judged disproportionate.
- A fork release writer can still replace the launcher and its baked-in binary
  digest table. The daily audit detects that condition after publication; it
  does not protect users who launch the replacement first.
- `.github/workflows/pinned-artifacts.yml` has `issues: write` only for its
  digest job. On failure it creates or comments on the fixed
  `[security] Pinned Slack MCP launcher digest audit failed` issue. The issue is
  never auto-closed after a green run, so investigation remains explicit.

Reopen prevention as soon as maintainers obtain a fork-controlled npm package
name and configure trusted launcher publication. Move the catalogue entry to an
exact registry version, confirm `dist.integrity`, switch Renovate to the npm
source, and update the source constant, tests, and both documentation layers in
the same reviewed change. Asset signing remains complementary: verification
inside the current launcher starts too late to protect that launcher itself.

## Repository-side guardrails

- A shape test beats a string test: parse the URL into
  `owner/repo/tag/asset`, assert `owner/repo` equals the owner/repo in
  `meta.<server>.url`, assert `tag` equals a named constant, and reject
  `@latest`, `#master`, `refs/heads/`, `/archive/`. That test fails for the
  *next* person who reaches for a mutable pin, not just for today's.
- Renovate needs `ignoreUnstable: false` for fork tags like `v1.3.0-vk.1`:
  they are semver **prereleases**, and the default stability filter makes the
  manager match the pin and then never propose anything — coverage that looks
  real and is not.
- A GitHub release datasource can return `v`-prefixed tags while the URL's
  captured `currentValue` omits that prefix. Use
  `extractVersionTemplate: ^v?(?<version>.*)$`: Renovate applies it to datasource
  versions, and accepting both shapes makes the comparison explicit and robust.
- When a version appears twice in one URL (release tag *and* tarball filename),
  the custom manager needs **two** `matchStrings` capturing `currentValue`, or a
  bump rewrites half the URL into a 404.
- Scope pre-existing `packageRules` with `matchFileNames` when adding a second
  dep to the same datasource, or the older rule's `prBodyNotes` will give
  confidently wrong instructions about the new one.
- A notification step after a failing audit must include a GitHub status
  function, for example
  `if: failure() && steps.digest.outcome == 'failure'`. Without `failure()`,
  Actions implicitly gates the expression on `success()`, so the very failure
  that should create the incident skips the notification step.

## Building the artifact

An operator-owned Nix deployment can close the outer-tarball gap that generic
`npx` installs retain: fetch the launcher URL as a fixed-output derivation, then
pass the verified store path to `npx`. Keep that deployment URL/hash in the same
coordinated review as the catalog tag and digest. If an stdio-to-HTTP migration
recognizes shipped launchers, its historical list is append-only so a later pin
bump does not strand old credential-bearing native configs.

- `npm pack` on a **scoped** package emits `scope-name-version.tgz`; if the pin
  says otherwise the install 404s. Keep the launcher unscoped so the packed
  filename *is* the pinned filename.
- Build with `CGO_ENABLED=0 -trimpath` and take the build timestamp from the
  tagged commit rather than the wall clock: rebuilding the tag then reproduces
  the published digests byte for byte (worth verifying by building twice — it
  is the difference between "pinned" and "reproducible"). Pin `TZ` while doing
  it: `git show --date=format-local` renders in the *builder's* timezone, so
  "same tag, same digest" silently holds only on machines sharing your offset.
  Verify by building the same tag under two different `TZ` values, not twice on
  the same machine.
- A fork inherits upstream's tag-triggered release workflows, which publish
  under **upstream's** npm and Docker Hub names. Disarm them
  (`tags-ignore: ['*-vk.*']`) before the first fork tag exists, not after.
- Version scheme `v<upstream-base>-vk.<n>` states what the fork is based on and
  never collides with an upstream tag. Assets are immutable; a correction is
  `-vk.<n+1>`.

## Verifying it for real

Cache isolation is the whole point of the exercise: run with a fresh
`npm_config_cache` **and** a launcher cache directory that does not exist, or a
warm cache will happily prove the previous artifact works. Then drive the server
over stdio (`initialize` → `notifications/initialized` → `tools/list`) — the
`initialize` response's `serverInfo.version` is where the stamped build version
surfaces when the binary has no `--version` flag, which is common.

An "unknown tool" error and a handler-level argument error look similar from a
distance and mean opposite things: `file_id is required` proves the tool is
registered and its handler ran. Read the message before concluding the tool is
missing.
