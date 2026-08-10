# Responsive deployment identity

Contributing tasks: `vk/6e4c-deploy-status-mo`.

## Responsive shells can drop operational data

Vibe Kanban does not merely resize one desktop shell. `SharedAppLayout` renders
the desktop `AppBar` or the mobile `NavbarContainer` as separate compositions.
Metadata shown only by the desktop rail therefore disappears at the mobile
breakpoint unless it is deliberately passed into the mobile navbar.

When operational identity matters for debugging, treat responsive replacement
shells as separate consumers of the same view model. Keep data access in
`packages/web-core`; add optional presentation props/components in `packages/ui`.

## Reuse revision identity and add lifecycle time to `/api/info`

The running revision already has one authoritative application convention:
`crates/server/build.rs` stamps `VK_GIT_SHA`, and `UserSystemInfo.version`
returns it with `dev` as the unstamped sentinel. Mobile and desktop must reuse
that value and the same commit-link convention.

Elapsed deployment age cannot come from browser mount time (reload resets it),
Git commit time (source history may predate deployment), or build time (health-
gated activation may happen later). A lightweight service-owned approximation
is one `DateTime<Utc>` initialized when Axum constructs the config router and
returned as additive `UserSystemInfo.started_at`. It remains stable for the
process lifetime and truthfully resets if the deployed service restarts.

Contract changes flow from the Rust `TS` declaration through
`pnpm run generate-types`; never hand-edit `shared/types.ts`. During rolling or
mixed-version transitions, frontend state should still normalize an absent
runtime field to `null`, even when the newly generated compile-time type marks
it required.

## Compact age labels and test ownership

For a constrained header, completed single units (`now`, `Nm`, `Nh`, `Nd`) are
more stable than sentence-form relative time. Recompute at the finest displayed
cadence (one minute here), clean up the interval on unmount, and render whichever
metadata is valid rather than failing the entire status item.

`packages/ui` has typecheck/lint scripts but no package-local Vitest lane.
Presentational helpers can remain owned and exported by `packages/ui` while
render/format coverage lives in an established consumer lane such as
`packages/web-core`, which resolves `@vibe/ui` through the workspace graph.

One generator-specific verification detail: `shared/types.ts` currently emits
trailing spaces on multiline declarations. `pnpm run generate-types:check` is
the authority for generator fidelity; a scoped `git diff --check` can exclude
that generated file while still checking all authored files.
