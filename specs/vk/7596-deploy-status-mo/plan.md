# Implementation Plan: Mobile Deploy Status

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

- Backend: Rust/Axum in `crates/server`; build metadata is stamped by `crates/server/build.rs` and returned from `crates/server/src/routes/config.rs` via `GET /api/info`.
- Build/release: `local-build.sh` compiles the frontend and Rust binaries, then writes the immutable release manifest. Its `built_at` value is the deployment-age source.
- Shared contract: `UserSystemInfo` derives `TS`; `shared/types.ts` is generated through `pnpm run generate-types`.
- Frontend: React/TypeScript. `packages/web-core` owns data containers; `packages/ui` owns `Navbar` presentation. `SharedAppLayout` selects the responsive mobile path.
- Constraints: no new dependency; preserve `dev` and missing-metadata compatibility; existing update polling remains separate.

## Architecture & Approach

1. In `local-build.sh`, calculate one UTC ISO-8601 timestamp before any frontend/backend compilation, export it as `VK_BUILD_TIMESTAMP`, and reuse it in `release.json`. This makes the embedded value and manifest value identical.
2. In `crates/server/build.rs`, rerun when `VK_BUILD_TIMESTAMP` changes and embed a non-empty value as a Rust compile-time environment variable.
3. In `crates/server/src/routes/config.rs`, add optional `deployment_timestamp` to `UserSystemInfo` and populate it from the embedded build value. Unstamped builds return `null`.
4. Register/regenerate the TS contract through the existing generator.
5. In `useUserSystemController.ts` and `useUserSystem.ts`, expose the timestamp beside `appVersion`; containers remain the data owner.
6. Add a pure deploy-age formatter and a presentational deploy-status component in `packages/ui`, with deterministic unit tests. The component owns a minute-scale tick so its relative label advances without data refetch.
7. Extend `Navbar` with optional mobile deployment props. Render the compact indicator only in mobile mode, retain the revision at the narrowest widths, and hide elapsed time first.
8. Extend `NavbarContainer` props and `SharedAppLayout` wiring so the already-loaded system metadata reaches only the responsive mobile navbar. Do not fetch `/api/info` again.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/user-system-info.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- **II Test the contract**: pure formatter tests, rendered component coverage, generated-contract checks, and backend response/type verification are planned.
- **III Small, reversible steps**: adds one optional field and reuses the existing `/api/info` load and deployment release timestamp.
- **IV Shared-component boundaries**: `web-core` supplies data; `packages/ui` owns the mobile header's rendering.
- **VI Don't rebuild what shipped**: reuses `release.json.built_at`, existing Git SHA stamping, `useUserSystem`, and desktop commit-link semantics.
- **XIV Worktree-safe verification**: locked pnpm setup precedes repository checks.
- Generated files are updated only by the generator, and `pnpm run format` is included.
- No constitution deviation or new dependency.

## Risks & Dependencies

- Cargo may reuse an old build-script output unless `rerun-if-env-changed` is emitted for the timestamp; the plan explicitly adds it.
- The local build creates frontend assets before Rust stamping, but only the backend needs the timestamp because the loaded system context supplies it at runtime.
- The remote deployment may expose `UserSystemInfo` differently; generated types and both frontend type checks guard the shared blast radius.
- Dense phone headers can overflow. Status uses shrink-safe classes, revision-first priority, and representative viewport inspection.
