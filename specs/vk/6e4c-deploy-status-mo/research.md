# Research: Mobile Deploy Status

## Existing revision source

`crates/server/build.rs` resolves `git rev-parse --short HEAD` and stamps it as
`VK_GIT_SHA`. `UserSystemInfo.version` returns that value, falling back to
`dev`, and the desktop `AppBar` already renders/linkifies it. This is the one
revision convention to reuse.

## Existing responsive composition

`SharedAppLayout` renders `AppBar` on desktop and `NavbarContainer` on mobile.
The mobile `Navbar` top row owns a right-side utility cluster. Since the desktop
rail is absent, passing operational metadata into this existing cluster is the
smallest layout change that restores parity.

## Timestamp choice

Alternatives rejected:

- Browser mount/fetch time resets on reload and understates deployment age.
- Git commit time describes source history, not when this process became live.
- Build time may substantially precede successful health-gated activation.
- A homelab release-manifest API would expand the task into infrastructure and
  duplicate system information already returned by the service.

Chosen: one UTC timestamp captured by `local-build.sh`, exported to the server
build as `VK_BUILD_TIMESTAMP`, and written to `release.json`. `/api/info`
returns the embedded optional timestamp. It is stable across browser and
service restarts and keeps runtime status aligned with release metadata.

## Formatting choice

Use `now`, `Nm`, `Nh`, and `Nd`, flooring completed units and clamping future or
invalid ages to a safe result. Recompute at one-minute cadence. No date library
or new dependency is needed.
