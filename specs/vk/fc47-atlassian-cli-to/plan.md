# Technical Plan: Add Atlassian CLI to Managed CLI Tools

**Feature dir**: `specs/vk/fc47-atlassian-cli-to/`
**Task**: `vk/fc47-atlassian-cli-to`
**Spec**: [`spec.md`](spec.md)

## Approach

Add Atlassian CLI as one more entry in the existing app-managed CLI catalog in
`crates/services/src/services/cli_tools.rs`. Reuse the current archive download,
SHA-256 verification, staged extraction, atomic symlink exposure, status, remove,
host-copy detection, and spawned-agent PATH behavior. No new installer surface,
credential store, executor, database schema, or frontend-specific Atlassian
workflow is required.

The only user-visible new value should be a managed tool with wire id `acli`,
binary name `acli`, display name `Atlassian CLI`, pinned version
`1.3.22-stable`, and official Atlassian documentation links surfaced through the
existing CLI Tools settings section.

## Grounding

- `crates/services/src/services/cli_tools.rs`
  - `CliToolId`, `CliToolId::ALL`, `dir_name()`, `catalog()`, and existing
    catalog tests are the source of truth for managed tool identity and
    metadata.
  - `InstallStrategy::ArchiveBinary` with `ArchiveKind::TarGz` already matches
    the Atlassian Linux tarball shape.
  - The current `ArchiveBinary` metadata stores one catalog-level
    `binary_path_in_archive`; ACLI needs a generic per-platform archive binary
    path because Atlassian's amd64 and arm64 tarballs extract different
    top-level directory names.
  - `platform_source()`, `unsupported_reason()`, `install_archive()`,
    `promote_staged_version()`, `bin_link_path()`, and `detect_host_copy()`
    already provide the required platform gating, verification, atomic exposure,
    removal/update behavior, and host-copy precedence.
- `crates/server/src/routes/cli_tools.rs`
  - Existing routes (`GET /api/cli-tools`, `POST /api/cli-tools/{id}/install`,
    `POST /api/cli-tools/{id}/update`, `DELETE /api/cli-tools/{id}`) need no
    route-level change beyond accepting the new generated enum value.
- `crates/server/src/bin/generate_types.rs`
  - Already exports `CliToolId` and `CliToolStatus`; regenerate shared types
    after extending the Rust enum.
- `packages/web-core/src/shared/dialogs/settings/settings/CliToolsSettingsSection.tsx`
  - Renders the server-provided catalog generically. It should display ACLI
    automatically once the API returns it.

## Implementation Steps

1. Extend `CliToolId` with `Acli`, add it to `CliToolId::ALL`, and map
   `dir_name()` to the stable wire/directory id `acli`.
2. Add `const ACLI_VERSION: &str = "1.3.22-stable";` near the other version
   pins. Do not use a moving `latest` URL.
3. Generalize archive metadata so the selected platform source can carry the
   executable path inside the extracted archive. Migrate existing archive tools
   to preserve their current paths; do not add an ACLI-specific branch.
4. Add a `CliToolCatalogEntry` for Atlassian CLI:
   - `binary_name`: `acli`
   - `display_name`: `Atlassian CLI`
   - `description`: concise Atlassian Cloud workflow text
   - `version`: `ACLI_VERSION`
   - `version_args`: `&["--version"]`
   - `strategy`: `ArchiveBinary { archive: ArchiveKind::TarGz }`, with the
     exact extracted executable path carried by the selected `PlatformSource`
     or equivalent generic per-platform source metadata.
   - `docs_url`: use Atlassian's official ACLI install guide as the primary
     settings link.
5. Add Linux platform sources only:
   - Linux x86-64:
     `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_amd64.tar.gz`
     with SHA-256
     `de9e0a60a556e4119428b9072f6ca787e75b9f9a538aa71ebcc8084deb8ca1a6`.
     Archive executable path:
     `acli_1.3.22-stable_linux_amd64/acli`.
   - Linux arm64:
     `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_arm64.tar.gz`
     with SHA-256
     `1a9e86d0b46a62a8f1992c1ef98b3af7e9a9ee3f76d0efa215fe1f2d1b2fd139`.
     Archive executable path:
     `acli_1.3.22-stable_linux_arm64/acli`.
6. Preserve unsupported-host behavior by relying on `platform_source()` returning
   no match outside Linux `x86_64` and `aarch64`.
7. Extend focused service tests:
   - catalog covers every id exactly once after adding `Acli`;
   - `acli` serializes/deserializes as `acli`;
   - ACLI entry has exact display name, binary name, pinned version, docs URL,
     Linux source URLs, SHA-256 values, `TarGz` archive kind, and expected
     version probe args;
   - unsupported platform behavior remains generic and no existing tool
     metadata is changed unintentionally.
8. Regenerate local shared types with `pnpm run generate-types` so
   `shared/types.ts` includes the new `CliToolId` union member. Do not edit the
   generated file by hand.
9. Run validation:
   - `cargo test -p services cli_tools`
   - `pnpm run generate-types:check`
   - `pnpm run check`
   - `pnpm run format`
   - Optional acceptance/network check on supported Linux:
     `cargo test -p services -- --ignored cli_tools` after extending the ignored
     end-to-end test to include `CliToolId::Acli` or adding an ACLI-specific
     ignored test.
10. After implementation, document the reusable managed-CLI catalog extension
   process in the project knowledge base as called out by
   [`../../../PRIOR_KNOWLEDGE.md`](../../../PRIOR_KNOWLEDGE.md).

## Contracts

No new endpoint is needed. The existing CLI Tools API accepts and returns a new
`CliToolId` value, `acli`. See [`contracts/cli-tools.md`](contracts/cli-tools.md).

## Data Model

No database model change. The relevant state is the existing managed-tool
catalog entry, generated TypeScript enum/union value, app-owned filesystem
layout, symlink exposure, and install manifest. See [`data-model.md`](data-model.md).

## Constitution Check

- **I Clarity over cleverness**: add a straightforward catalog entry and tests;
  any archive-layout edge is isolated to the catalog/install strategy.
- **II Test the contract**: focused service tests cover identity, metadata,
  platform sources, checksum pins, and wire id; optional ignored network test
  covers real vendor install/remove.
- **III Small, reversible steps**: one catalog extension, one minimum generic
  archive metadata adjustment, generated type update, no new workflow.
- **IV Shared-component boundaries**: no UI component change expected; the
  shared settings section already renders catalog entries from API data.
- **V Remote mutations**: not applicable; no `crates/remote` mutation or
  ElectricSQL contract change.
- **VI Don't rebuild what shipped**: reuse the existing managed-tools installer,
  status model, and settings UI.
- **VII Managed tools are pinned, verified, and user-owned**: pinned
  `1.3.22-stable`, per-platform SHA-256 values, official docs, app-owned install
  location, existing staged atomic exposure, and no Atlassian credential
  handling.

## Risks

- Atlassian tarballs contain an architecture-specific top-level directory. The
  current single static `binary_path_in_archive` cannot represent both `amd64`
  and `arm64`, so the implementation must let `PlatformSource` or equivalent
  generic source metadata carry the per-platform binary path. Keep that change
  generic for archive tools and covered by tests.
- The checksums were computed from Atlassian-hosted artifacts because adjacent
  checksum files were not available. Future ACLI bumps require recomputing and
  reviewing both hashes before merge.
- macOS and Windows are intentionally unsupported in this release; adding them
  later requires separate vendor artifact URLs, checksums, and validation.

## Rollback

Revert the `CliToolId::Acli` enum member, ACLI catalog entry, generated
`shared/types.ts` change, and ACLI-specific tests. Existing installed tools and
wire ids are otherwise untouched.
