# Self-Hosted Deployment: Versioned Releases & the think2 Pipeline

How this fork deploys itself, and the hard-won invariants behind the design.
The deploy machinery lives in the homelab repo
(`modules/vibe-kanban-rebuild.nix` and friends); this repo's contribution is
`local-build.sh`'s release publishing.

## The release contract (`VK_RELEASES_DIR`)

When `VK_RELEASES_DIR` is set, `local-build.sh` publishes every build as an
immutable, versioned release:

```
$VK_RELEASES_DIR/
  build-<id>/bin/{vibe-kanban,vibe-kanban-mcp,vibe-kanban-review,remote,relay-server}
  build-<id>/release.json     {"sha","build_id","built_at"}
  current  -> build-<id>      # atomic rename flip
  previous -> old current     # one-step rollback target
```

Unset (CI, local dev), the build behaves exactly as before. Key properties:

- **Extracted binaries, not zips** — services exec `current/bin/*` directly;
  no launcher, no extraction, no writes at service start.
- **Self-describing** — `release.json.sha` lets a reconciler compare
  "deployed" to "desired" mechanically. It equals the stamp SHA because the
  deploy host builds a worktree pinned to the stamp.
- **Stage/flip split** — binaries are fully staged before the atomic `current`
  flip, so a mid-script failure leaves the previous complete release live and
  the drift remains detectable via `release.json`.
- **Locked flip+prune** — `previous`-repoint, `current`-flip, and prune run
  under `$VK_RELEASES_DIR/.publish-lock` (concurrent builders race
  otherwise), and prune never deletes the resolved targets of
  `current`/`previous`.
- **No remote-web publish** — live UI traffic is served by local-web through
  the path-splitting Caddy listener. The remote binary handles only API paths,
  so `/srv/static` is not part of the release or rollback verdict. Remote-web
  retains its independent frontend CI build and tests.

## Surfacing deployed identity in the UI

Deployment identity should come from the immutable artifact, not from the
service process. `local-build.sh` now calculates one UTC timestamp before the
release build, exports it as `VK_BUILD_TIMESTAMP` for the server build script,
and writes the identical value to `release.json.built_at`. The server exposes
that optional timestamp beside its embedded `VK_GIT_SHA` through `/api/info`.

This creates one convention for “time since deployed”:

- it means time since the running immutable release was built/published;
- restarting the service does not reset it, because a restart is not a deploy;
- unstamped and older builds return no timestamp, so clients retain revision
  identity without fabricating an age;
- the browser derives relative age from the timestamp and may update it locally
  without polling a second deployment endpoint.

Keep the value optional in API contracts. Release-path access is not guaranteed
for every packaging mode, and reading `current/release.json` on each request
would couple the server to one host layout unnecessarily.

## Why services must not run from the source checkout

`/srv/src/vibe-kanban` on the deploy host is simultaneously: poll target
(hard-reset every 15 min), app git workspace (task worktrees register under
`.git/worktrees/`, owned by the service user), and build input. When it was
*also* the artifact store (`npx-cli/dist`, re-extracted by `cli.js` on every
service start):

- a wiped/failed `dist` crash-looped the services (the original CI-runner
  incident, and again with `git worktree prune` dying on foreign-owned
  worktree registrations — `Permission denied`, observed 2026-07-05);
- service users needed group write access to the repo *for startup*;
- `git-projects-fix-permissions` chmod sweeps stripped +x off deployed
  binaries, requiring root `ExecStartPre chmod` hacks in the units.

All three go away when artifacts live outside `/srv/src` behind a symlink.

## Deploy-loop invariants (learned the hard way)

1. **Edge triggers stall silently.** The stamp PathChanged trigger misses
   events (build killed mid-run, exit 127) and a failed build leaves the
   stamp unchanged — nothing re-fires until the next upstream commit. A
   periodic reconciler comparing stamp SHA to deployed `release.json` SHA is
   the only reliable convergence mechanism. Existence checks are not enough:
   binaries from commit N−1 are "present" while commit N is silently
   undeployed.
2. **Failures must page.** `OnFailure=` → ntfy on the build unit, plus a
   stalled-deploy page from the reconciler (entering-stall + bounded
   reminders, not one page per cycle).
3. **Health-gate restarts, and treat a failed `systemctl restart` as a
   failed health check** — under `set -e` it would otherwise abort before
   the rollback runs, leaving `current` on the broken release.
4. **Never `git worktree prune` a checkout the app also uses** — it walks
   every registration including the app's task worktrees (different owner →
   `Permission denied` → dead build). Scope cleanup to the worktree you
   created (`worktree remove --force` + rm of your own
   `.git/worktrees/<name>` admin dir).
5. **Cross-repo sequencing needs a guard.** The homelab deployer verifies
   `release.json.sha == stamp sha` after the build and fails with an
   actionable message — protecting against a deploy branch whose
   `local-build.sh` predates `VK_RELEASES_DIR` support.

## Health endpoints

All three deployed binaries already expose probes (no server changes were
needed): `server` → `/api/health` (mounted under `/api`; note the bare
`/health` on the local instance is answered 200 by the SPA fallback, so
always probe the API route), `remote` → `/health`, `relay-server` →
`/health`. A 200 from the server implies startup init (including DB
migrations) completed, because the listener binds only after init.

## Rejected alternatives (and why)

- **Nix-native flake package + comin**: no incremental cargo cache in the
  sandbox (5–30 min warm builds become cold every time), heavy
  crane/naersk + pnpm-fetcher machinery, and app deploys would couple to
  homelab flake bumps.
- **Containers**: the app's job is spawning agents against host checkouts,
  SSH, and host toolchains; a container boundary multiplies mount/permission
  complexity for no isolation win.
- **Blue/green**: SQLite single-writer; restart-based deploys with
  health-gate + rollback cover the real risk (bad release), not the
  seconds-long restart blip.

## Contributed by

- vk/f00d-vibe-kanban-depl
- vk/7596-deploy-status-mo
