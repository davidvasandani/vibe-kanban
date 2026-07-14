# Analysis — Atlassian CLI Managed Tool Planning Artifacts

## Scope

Reviewed these planning artifacts against
[`../../../.specify/memory/constitution.md`](../../../.specify/memory/constitution.md):

- [`spec.md`](spec.md)
- [`plan.md`](plan.md)
- [`research.md`](research.md)
- [`data-model.md`](data-model.md)
- [`contracts/cli-tools.md`](contracts/cli-tools.md)
- [`tasks.md`](tasks.md)

No product code was changed.

## Fixes Applied

- Resolved the archive-path planning ambiguity. The current managed CLI catalog
  has one catalog-level `binary_path_in_archive`, but ACLI's Linux amd64 and
  arm64 archives extract different top-level directories. The plan, research,
  data model, and task list now explicitly require a generic per-platform
  archive binary path on `PlatformSource` or equivalent source metadata.
- Removed unsafe `[P]` markers from tasks that edit
  `crates/services/src/services/cli_tools.rs`. Those same-file edits are now
  dependency-ordered to avoid conflicting concurrent changes.
- Fixed stale knowledge-base paths. References now point to the existing
  repository root `PRIOR_KNOWLEDGE.md` instead of nonexistent
  `../PRIOR_KNOWLEDGE.md` or `specs/vk/PRIOR_KNOWLEDGE.md`.

## Constitution Assessment

- **I. Clarity over cleverness**: Pass. The artifacts now state the required
  generic archive metadata change directly instead of leaving it as a conditional
  implementation detail.
- **II. Test the contract**: Pass. Acceptance criteria and tasks require focused
  service tests, generated type checks, and optional ignored vendor install
  validation.
- **III. Small, reversible steps**: Pass. Scope remains a managed catalog
  extension plus the minimum generic archive metadata change required by ACLI's
  artifact layout.
- **IV. Shared-component boundaries are law**: Pass. No shared UI component
  change is planned; the existing `web-core` settings section is read-only
  verification.
- **V. Remote mutations are transactional and txid-covered**: Not applicable.
  No `crates/remote` mutation or ElectricSQL workflow is in scope.
- **VI. Don't rebuild what shipped**: Pass. The artifacts reuse the existing CLI
  Tools API, installer lifecycle, status model, and spawned-agent PATH behavior.
- **VII. Managed tools are pinned, verified, and user-owned**: Pass. The spec
  pins `1.3.22-stable`, records exact per-platform SHA-256 values, keeps
  credentials host-owned, and relies on staged atomic install behavior.

## Remaining Risks

- The recorded ACLI SHA-256 values were computed from Atlassian-hosted artifacts
  because adjacent vendor checksum files were not available. Future version
  bumps require human review and refreshed checksums before merge.
- macOS and Windows remain intentionally unsupported until separate pinned
  vendor artifacts and checksums are selected.

## Final Assessment

The reviewed planning artifacts are now internally consistent and aligned with
the project constitution. No open constitution violations, wrong paths, unsafe
parallel markers, or unresolved planning contradictions remain in the reviewed
set.
