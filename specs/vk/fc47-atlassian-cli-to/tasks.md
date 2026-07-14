# Tasks — Add Atlassian CLI to Managed CLI Tools

**Task**: `vk/fc47-atlassian-cli-to` · **Plan**: [`plan.md`](plan.md)

`[P]` = parallelizable within its dependency layer. Parallel tasks either touch
different files, are read-only verification, or are validation commands after
the same prerequisite. Tasks that edit the same file are intentionally not
marked parallel.

## Dependency-Ordered Tasks

| ID | Task | File(s) | Depends on |
| --- | --- | --- | --- |
| [x] T001 | Add `CliToolId::Acli`, include it in `CliToolId::ALL`, and map `dir_name()` to the stable wire/directory id `acli`. | `crates/services/src/services/cli_tools.rs` | — |
| [x] T002 | Add `ACLI_VERSION: &str = "1.3.22-stable"` near the existing managed-tool version pins. | `crates/services/src/services/cli_tools.rs` | — |
| [x] T003 | Generalize archive metadata so an archive install can resolve the executable path from the selected `PlatformSource`, preserving existing paths for current tools. Do this generically; do not add an ACLI-specific branch. | `crates/services/src/services/cli_tools.rs` | — |
| [x] T004 | Update archive install/status helpers to use the selected platform source's binary path when validating extraction and building the installed binary path. | `crates/services/src/services/cli_tools.rs` | T003 |
| [x] T005 | Add the Atlassian CLI catalog entry with binary `acli`, display name `Atlassian CLI`, Atlassian Cloud description, `["--version"]`, official install guide docs URL, `TarGz` strategy, and Linux x86-64 / arm64 platform sources with exact pinned URLs, SHA-256 values, and per-platform archive binary paths. | `crates/services/src/services/cli_tools.rs` | T001, T002, T004 |
| [x] T006 | Add focused catalog tests covering ACLI identity, serde wire id `acli`, display/binary names, pinned version, docs URL, version args, archive kind, Linux-only platform matrix, exact URLs, exact SHA-256 values, and per-platform archive paths. | `crates/services/src/services/cli_tools.rs` | T005 |
| [x] T007 | Add or extend generic archive metadata tests so existing managed tools keep their current archive binary paths and platform source behavior after T003/T004. | `crates/services/src/services/cli_tools.rs` | T006 |
| [x] T008 | Extend unsupported-platform/status tests or assertions to prove ACLI relies on the existing unsupported-host behavior outside Linux `x86_64` and `aarch64`, without changing existing tool behavior. | `crates/services/src/services/cli_tools.rs` | T007 |
| [x] T009 | Extend the ignored Unix end-to-end vendor install/remove test, or add an ACLI-specific ignored test, to cover real ACLI download, checksum verification, executable exposure, version probe, idempotent reinstall, staging cleanup, and removal. | `crates/services/src/services/cli_tools.rs` | T008 |
| [x] T010 | Regenerate local shared TypeScript types so `CliToolId` includes `"acli"`; do not hand-edit generated output. | `shared/types.ts` | T005 |
| [x] T011 | [P] Verify the existing CLI Tools routes accept `acli` through generated enum deserialization and need no route-level change. | `crates/server/src/routes/cli_tools.rs` | T010 |
| [x] T012 | [P] Verify the existing CLI Tools settings section renders the server-provided ACLI row generically and needs no Atlassian-specific UI change. | `packages/web-core/src/shared/dialogs/settings/settings/CliToolsSettingsSection.tsx` | T010 |
| [x] T013 | [P] Run focused Rust tests for managed CLI tools. | — | T006, T007, T008 |
| [x] T014 | [P] Run generated-type verification. | — | T010 |
| T015 | [P] Run the ignored ACLI/vendor install acceptance test on a supported Linux host when network validation is desired. | — | T009 |
| [x] T016 | Run repository-wide checks required for this change. | — | T013, T014 |
| [x] T017 | Run repository formatting before completion. | — | T016 |
| [x] T018 | Document the reusable managed-CLI catalog extension process in the project knowledge base. | `wiki/managed-cli-tool-catalog.md`, `wiki/INDEX.md` | T017 |

## Suggested Commands

- `pnpm run generate-types`
- `cargo test -p services cli_tools`
- `pnpm run generate-types:check`
- `pnpm run check`
- `pnpm run format`
- Optional supported-Linux network acceptance:
  `cargo test -p services -- --ignored cli_tools`

## Notes

- No database migration, new API route, Atlassian credential handling, executor,
  or frontend-specific Atlassian workflow is part of this feature.
- Existing managed CLI tool identifiers, metadata, install state, remove
  behavior, and PATH precedence must remain unchanged.
- `shared/types.ts` is generated output; the implementation source of truth is
  `crates/services/src/services/cli_tools.rs` plus
  `crates/server/src/bin/generate_types.rs`.
