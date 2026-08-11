# Implementation Plan: Mobile Deploy Status

**Spec**: `./spec.md`
**Status**: Implemented

## Technical Context

The local server is Rust/Axum. `crates/server/src/routes/config.rs` serves
`GET /api/info` as `UserSystemInfo`, whose `version` is the build-stamped
`VK_GIT_SHA`. Rust declarations generate `shared/types.ts` through
`pnpm run generate-types`. React/TypeScript containers in
`packages/web-core` consume this response; presentational application chrome is
owned by `packages/ui`. `SharedAppLayout` selects the mobile `NavbarContainer`
instead of the desktop `AppBar` below the existing mobile breakpoint.

## Architecture & Approach

1. In `local-build.sh`, capture one UTC release timestamp, export it for the
   server build, and write the same value to `release.json`. In
   `crates/server/build.rs` and `crates/server/src/routes/config.rs`, embed and
   expose it as optional `deployment_timestamp` on `UserSystemInfo`.
2. Regenerate `shared/types.ts` so `UserSystemInfo.deployment_timestamp` is
   consumed from the Rust source of truth.
3. Extend `UserSystemState` and `useUserSystemController` in
   `packages/web-core/src/shared/hooks/` to retain and expose the timestamp
   alongside the existing `appVersion` SHA.
4. Pass both values through `NavbarContainer` to a new optional mobile
   deployment-status slot/props on `packages/ui/src/components/Navbar.tsx`.
   Keep data access in `web-core` and presentation in `packages/ui`, matching
   the shared-component boundary.
5. Implement a small presentational mobile status component/helper in
   `packages/ui` that displays `SHA · age`, links production SHAs to the same
   commit URL used by `AppBar`, and recomputes compact age once per minute.
   Render it only in mobile mode and only when at least one usable value exists.
6. Add unit coverage for time-boundary formatting and rendered mobile status
   semantics at the nearest package with an established Vitest setup. Confirm
   missing metadata and `dev` handling.
7. Run generated-type verification, targeted TypeScript tests/checks, relevant
   Rust tests/checks, and repository formatting.

## Data Model

See `./data-model.md`.

## Contracts

See `./contracts/system-info.md`.

## Research Notes

See `./research.md`. No new dependency is required.

## Constitution Check

- II, Test the contract: focused formatting/render and contract checks are
  planned.
- III and VI, Small/reuse: the existing `/api/info`, `appVersion`, mobile
  navbar, and desktop commit URL convention are extended rather than duplicated.
- IV, Shared boundaries: `web-core` owns system data; `packages/ui` owns navbar
  presentation.
- XXII, Responsive operational identity: mobile consumes the same SHA plus the
  release's authoritative stable build timestamp.
- Generated types will be regenerated, never hand-edited, and `pnpm run format`
  runs before completion.

No constitution deviations are known.

## Risks & Dependencies

- The right-side mobile header is space-constrained. Status must use `shrink-0`,
  compact typography, and terse units without creating page overflow.
- Build time can precede health-gated activation by the remaining build and
  restart duration, but it is the stable timestamp available inside the
  immutable release and remains aligned with `release.json`.
- Mixed frontend/backend versions may omit the additive timestamp temporarily;
  frontend handling must tolerate absence at runtime despite the generated type.
