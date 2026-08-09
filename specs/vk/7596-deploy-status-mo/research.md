# Research: Mobile Deploy Status

## Decision 1: Timestamp semantics

Use the immutable release build/publish timestamp, not service process start time.

`local-build.sh` already writes `release.json.built_at` immediately after a successful build and before the atomic release flip. The deployment documentation treats that release as the deployed artifact. A service restart does not deploy new code, so process uptime would answer a different question.

## Decision 2: Metadata transport

Extend `GET /api/info` rather than add an endpoint or read deployment files at request time.

The endpoint already returns the embedded revision, `useUserSystem` already loads it, and `SharedAppLayout` already consumes that context. An optional timestamp beside the revision is cohesive and works in packaged layouts that do not have `VK_RELEASES_DIR` at runtime.

## Decision 3: Build stamping

Calculate one timestamp at `local-build.sh` start, export it through the Rust build, and write the identical string to `release.json`.

Alternatives rejected:

- Runtime read of `/srv/vk-releases/current/release.json`: deployment-path-specific, adds I/O/error handling to a hot configuration route, and does not work for other package layouts.
- Server process start time: measures uptime, not deployment age.
- Independent `date` calls in build script and manifest publication: small but needless semantic drift.
- Git commit author/committer time: measures source history, not when that artifact was deployed.

## Decision 4: Responsive priority

Keep revision visible and hide elapsed age first on the narrowest screens. Existing actions remain higher priority than both. The revision is the primary evidence for whether a desired change is live; age is supporting context.

## Dependencies

No new package or crate dependency is needed. Date parsing and timers use browser APIs; UTC timestamp generation uses existing shell tools.

