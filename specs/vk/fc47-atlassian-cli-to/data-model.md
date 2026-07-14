# Data Model — Atlassian CLI Managed Tool

No database schema change. This feature extends existing in-process catalog and
filesystem-backed managed-tool state.

## `CliToolId`

Rust enum in `crates/services/src/services/cli_tools.rs`.

- Add variant: `Acli`
- Wire format: `acli` via `#[serde(rename_all = "kebab-case")]`
- Directory id: `CliToolId::Acli.dir_name() == "acli"`
- Include in `CliToolId::ALL` so list/status/lock coverage remains complete.
- Regenerate `shared/types.ts` so TypeScript clients accept the new id.

## `CliToolCatalogEntry`

New catalog entry:

- `id`: `CliToolId::Acli`
- `binary_name`: `acli`
- `display_name`: `Atlassian CLI`
- `description`: Atlassian Cloud command-line workflow text
- `version`: `1.3.22-stable`
- `version_args`: `["--version"]`
- `sources`: two Linux sources, one for `x86_64`, one for `aarch64`
- `strategy`: archive binary from tar.gz
- `docs_url`: official ACLI install guide

## `PlatformSource`

Existing URL/checksum/platform matching remains the right model:

- `os`: `linux`
- `arch`: `x86_64` or `aarch64`
- `url`: immutable Atlassian versioned tarball URL
- `sha256`: exact lowercase hex SHA-256
- `binary_path_in_archive`: exact executable path inside the selected extracted
  archive, or an equivalent generic per-platform source field

The current static catalog-level `binary_path_in_archive` cannot safely express
ACLI's per-architecture extracted top-level directory. Add a generic
per-platform binary path field to `PlatformSource` or equivalent source metadata
and migrate existing archive tools to keep their current paths. Do not add an
ACLI-only special case.

ACLI values:

- Linux `x86_64`: `acli_1.3.22-stable_linux_amd64/acli`
- Linux `aarch64`: `acli_1.3.22-stable_linux_arm64/acli`

## Filesystem State

Existing app-owned layout under `assets::cli_tools_dir()`:

- Tool root: `cli-tools/acli/`
- Version directory: `cli-tools/acli/1.3.22-stable/`
- Manifest: `cli-tools/acli/manifest.json`
- Exposed symlink: `cli-tools/bin/acli`
- Staging root: `cli-tools/.staging/acli/`

Only `cli-tools/bin` is appended to spawned-agent PATH, after host paths. A host
copy resolved earlier on PATH continues to take precedence.

## `InstalledManifest`

No shape change:

- `version`: `1.3.22-stable`
- `installed_at`: RFC3339 timestamp
- `verification`: `sha256:<selected-platform-sha256>`

The manifest is written only after download, checksum verification, extraction,
and staged promotion succeed. The `bin/acli` symlink is exposed last.

## `CliToolStatus`

No shape change. ACLI appears through the existing fields:

- `id`: `acli`
- `binary_name`: `acli`
- `display_name`: `Atlassian CLI`
- `description`: catalog description
- `catalog_version`: `1.3.22-stable`
- `supported`: true only on Linux x86-64 or Linux arm64
- `unsupported_reason`: existing platform message on unsupported hosts
- `host`: detected host-owned `acli`, excluding app-owned `cli-tools/bin`
- `app`: app-managed installed copy and outdated status
- `docs_url`: official Atlassian documentation link

## State Transitions

- **Not installed**: no app manifest and no app symlink. A host copy may still be
  reported independently.
- **Install/update requested**: download selected platform tarball into staging,
  verify SHA-256, extract, validate expected `acli` executable, promote the
  version directory, write manifest, then atomically expose `cli-tools/bin/acli`.
- **Install/update failure**: staging is cleaned best-effort; no partial app
  `acli` is exposed on PATH.
- **Outdated**: app manifest version differs from catalog version after a future
  pinned ACLI bump.
- **Remove requested**: remove app symlink and app-owned `cli-tools/acli/`
  directory; host-owned `acli` remains untouched.
