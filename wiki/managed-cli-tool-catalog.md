# Managed CLI Tool Catalog

Vibe Kanban's app-managed command-line tools are defined centrally in
`crates/services/src/services/cli_tools.rs`. The server routes and CLI Tools
settings UI consume that catalog generically, so adding a tool normally does
not require tool-specific route or frontend code.

## Catalog extension contract

Adding a managed tool requires all of the following to stay aligned:

- Add a `CliToolId` variant, include it in `CliToolId::ALL`, and give it a
  stable kebab-case `dir_name()`. Serde uses that value as the API wire id.
- Add one catalog entry with display metadata, a pinned version, version-probe
  arguments, vendor documentation, install strategy, and supported platform
  sources.
- Pin every downloadable artifact with a SHA-256 digest. Prefer immutable,
  version-addressed vendor URLs; a version bump must include refreshed hashes.
- Run `pnpm run generate-types` rather than editing `shared/types.ts` by hand.
  The generated `CliToolId` union is what frontend clients use.

`CliToolId::ALL` drives listing, per-tool locks, and catalog completeness tests.
Forgetting to add a new variant there makes the tool effectively invisible even
if a catalog entry exists.

## Archive layouts are platform metadata

An archive's executable path belongs to `PlatformSource`, not to the shared
`ArchiveBinary` strategy. Vendors can package different architectures under
different top-level directory names; Atlassian ACLI, for example, uses an
architecture-specific directory in each Linux tarball. Installation and
installed-copy detection must use the path from the selected source.

Keep this generic. Do not add vendor-specific extraction branches when the
difference can be expressed as source metadata. Regression tests should assert
that existing tools retain their current internal executable paths when this
metadata changes.

## Platform support and validation

Support is determined by an exact `(std::env::consts::OS,
std::env::consts::ARCH)` source match. Omitting a source intentionally reuses
the existing unsupported-platform status and install rejection behavior.

The practical validation sequence is:

1. Verify vendor archive URLs, SHA-256 values, internal executable paths, and
   executable permissions for every supported architecture.
2. Run `cargo test -p services cli_tools`.
3. Run `pnpm run generate-types:check`.
4. Run the repository checks and `pnpm run format`.
5. Keep a network-backed ignored install/remove test for real vendor artifact
   acceptance; run it deliberately because it downloads external tools.

The settings page renders catalog rows returned by the server and the routes
deserialize `CliToolId`, so confirm those paths remain generic before adding
frontend or route-level special cases.

## Workspace PATH propagation

Installing a tool and exposing it to workspace processes are separate
boundaries. “Available in a workspace” includes managed agent/script execution
and interactive workspace terminals; local and clustered variants of both must
be audited whenever the execution environment changes.

The canonical contract lives in `utils::shell::append_cli_tools_to_path`:

- derive `assets::cli_tools_dir()/bin` on the host that will spawn the child;
- add it only when that host's directory exists;
- append it after inherited PATH entries so a machine-provided copy wins;
- reuse `merge_paths` so custom entries survive and duplicates are removed.

Never send the coordinator's absolute app-data path to a cluster worker. CLI
Tools is machine-scoped, and worker state is node-local unless a separate
deployment contract proves otherwise. The worker augments execution and
terminal environments immediately before spawn; a missing worker-local install
is a no-op, not a dispatch failure.

Keep workspace-only policy out of generic PTY services. The local PTY service
also launches machine-scoped managed CLI login flows with a deliberately small
environment, so local workspace-terminal augmentation belongs in the terminal
route after the remote-worker branch has been selected.

## Contributed by

- vk/fc47-atlassian-cli-to
- vk/b2a2-add-vk-cli-tools
