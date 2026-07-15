# Research Notes — Atlassian CLI Managed Tool

## Inputs

- Clarified spec: [`spec.md`](spec.md)
- Constitution:
  [`../../../.specify/memory/constitution.md`](../../../.specify/memory/constitution.md)
- Prior knowledge: [`../../../PRIOR_KNOWLEDGE.md`](../../../PRIOR_KNOWLEDGE.md)
- Existing implementation plan:
  [`../../../IMPLEMENTATION_PLAN.md`](../../../IMPLEMENTATION_PLAN.md)
- Existing managed CLI implementation:
  `crates/services/src/services/cli_tools.rs`

## Decisions

### Decision: Reuse the managed CLI catalog

Add ACLI through `CliToolId`, `CliToolId::ALL`, and `catalog()`.

**Rationale**: The existing catalog already owns display metadata, docs links,
version pins, platform sources, install strategies, status reporting, and TS type
generation. The spec explicitly requires the same install/update/remove/inspect
workflow as existing tools.

**Alternatives rejected**:
- A separate Atlassian installer: duplicates lifecycle, PATH, unsupported-host,
  and checksum behavior.
- A frontend-only entry: would not provide install/remove/update behavior or
  generated API type coverage.

### Decision: Pin `1.3.22-stable`

Use `ACLI_VERSION = "1.3.22-stable"` and immutable versioned Atlassian URLs.

**Rationale**: The clarified spec records that Atlassian's Linux `latest`
artifact on 2026-07-14 unpacked to and reported `1.3.22-stable`, and that
Atlassian also serves matching versioned tarball URLs. A moving `latest` URL
would violate the managed-tools supply-chain contract.

### Decision: Support Linux `x86_64` and Linux `aarch64` only

Map Rust `std::env::consts::OS == "linux"` with architectures `x86_64` and
`aarch64` to Atlassian's Linux `amd64` and `arm64` tarballs respectively.

**Rationale**: These are the only platforms selected by the clarified spec and
the only artifacts with recorded checksums for this release.

**Consequence**: macOS, Windows, and other Linux architectures use the existing
unsupported-host response: `no <os>/<arch> build published by the vendor`.

### Decision: Use `ArchiveBinary` with `TarGz` and per-platform binary paths

ACLI should use the existing archive install path, with a generic metadata
extension so the selected platform source supplies the executable path inside
the extracted archive.

**Rationale**: The Atlassian artifacts are `.tar.gz` archives containing an
`acli` executable. The installer already verifies SHA-256 before extraction,
checks for the expected executable, promotes the extracted tree into the
app-owned version directory, writes the manifest, and exposes the symlink last.

**Consequence**: The extracted top-level directory name differs by vendor
architecture (`acli_1.3.22-stable_linux_amd64/acli` vs
`acli_1.3.22-stable_linux_arm64/acli`). Extend `PlatformSource` or equivalent
generic source metadata with a per-platform `binary_path_in_archive` instead of
adding an Atlassian-specific branch.

### Decision: Keep credentials host-owned

Do not add Atlassian token, site, login, or config storage.

**Rationale**: Existing managed tools expose binaries only; credentials and
configuration stay with the user/host. The spec and constitution both require
that ACLI authentication remains outside this feature.

### Decision: No new top-level dependency

Use the archive, checksum, and extraction crates already present in
`crates/services`.

**Rationale**: `tar`, `flate2`, `sha2`, `reqwest`, and the installer staging
logic already cover this feature. A dependency addition would add no value.

## Official References

Use these links in implementation/testing context:

- Primary catalog docs link:
  `https://developer.atlassian.com/cloud/acli/guides/install-acli/`
- Supplemental package/reference link:
  `https://developer.atlassian.com/cloud/acli/guides/download-supported-packages/`

## Artifact Matrix

| Host OS | Host arch | Vendor arch | URL | SHA-256 |
| --- | --- | --- | --- | --- |
| linux | x86_64 | amd64 | `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_amd64.tar.gz` | `de9e0a60a556e4119428b9072f6ca787e75b9f9a538aa71ebcc8084deb8ca1a6` |
| linux | aarch64 | arm64 | `https://acli.atlassian.com/linux/1.3.22-stable/acli_1.3.22-stable_linux_arm64.tar.gz` | `1a9e86d0b46a62a8f1992c1ef98b3af7e9a9ee3f76d0efa215fe1f2d1b2fd139` |

Archive executable paths:

| Host arch | Path inside extracted archive |
| --- | --- |
| x86_64 | `acli_1.3.22-stable_linux_amd64/acli` |
| aarch64 | `acli_1.3.22-stable_linux_arm64/acli` |

## Validation Research

- Existing unit tests already check catalog coverage, SHA-256 shape, HTTPS
  downloads, wire-id round trips, manifest round trips, invalid archive
  rejection, and tar.gz executable bit preservation.
- Add ACLI-specific assertions so a future refactor cannot silently change its
  pinned version, URLs, hashes, display metadata, docs link, or archive strategy.
- The ignored network install test is the right acceptance-test shape for real
  vendor downloads because it already exercises install, version probe,
  idempotent reinstall, staging cleanup, and remove.
