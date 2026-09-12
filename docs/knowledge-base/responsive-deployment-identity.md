# Responsive deployment identity

Contributing tasks: `vk/6e4c-deploy-status-mo`, `vk/7596-deploy-status-mo`,
`vk/0694-move-refresh`.

## Responsive shells can drop operational data

Vibe Kanban does not merely resize one desktop shell. `SharedAppLayout` renders
the desktop `AppBar` or the mobile `NavbarContainer` as separate compositions.
Metadata shown only by the desktop rail therefore disappears at the mobile
breakpoint unless it is deliberately passed into the mobile navbar.

When operational identity matters for debugging, treat responsive replacement
shells as separate consumers of the same view model. Keep data access in
`packages/web-core`; add optional presentation props/components in `packages/ui`.

## Reuse revision identity and stamp release time once

The running revision already has one authoritative application convention:
`crates/server/build.rs` stamps `VK_GIT_SHA`, and `UserSystemInfo.version`
returns it with `dev` as the unstamped sentinel. Mobile and desktop must reuse
that value and the same commit-link convention.

Elapsed deployment age cannot come from browser mount time (reload resets it)
or Git commit time (source history may predate deployment). The release build
captures one UTC `VK_BUILD_TIMESTAMP`, exports it so Cargo recompiles with that
value, and writes the same value to `release.json`. The server exposes it as the
additive optional `UserSystemInfo.deployment_timestamp`. This makes the UI and
release manifest describe the same immutable build without changing on browser
reload or ordinary process restart.

Contract changes flow from the Rust `TS` declaration through
`pnpm run generate-types`; never hand-edit `shared/types.ts`. During rolling or
mixed-version transitions, frontend state normalizes an absent runtime field to
`null`; the Rust contract also keeps the field optional so unstamped development
builds remain valid.

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

## Deployment actions belong with deployment identity

When the polled server revision diverges from the page-load revision, the
resulting page-reload action is deployment state, not account state. Keep that
Refresh action with Deploy Status rather than beneath the user avatar. Native
binary Update remains a separate AppBar concern because it invokes the desktop
updater instead of adopting a new web bundle.

If Deploy Status becomes one of the shared right-sidebar sections, preserve two
subtle contracts. First, insert it only after route-mode sections have been
assembled; otherwise later `unshift` operations can silently put Changes, Logs,
Preview, or Browser ahead of a supposedly first status section. Second, keep
revision/age in `headerExtra` so identity survives collapse, and use an isolated
section action for Refresh so mouse and keyboard activation do not toggle the
disclosure. Rendered-DOM tests should cover both ordering with a mode-specific
section present and action/disclosure isolation.
