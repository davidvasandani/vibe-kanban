# Spec: Production-Grade Deployment Strategy for Vibe Kanban

## Problem

Vibe Kanban self-hosts on think2 via a chain of homelab NixOS units:
`git-projects-update` (poll/CI-trigger) hard-resets `/srv/src/vibe-kanban` and
writes a SHA stamp → `vibe-kanban-rebuild.path` fires → `vibe-kanban-rebuild.service`
builds via `local-build.sh` in an isolated worktree → publishes `npx-cli/dist`
**back into the live checkout** → restarts `vibe-kanban-dev`, `vibe-kanban-remote`,
`vibe-kanban-relay`.

Two classes of production problems, both observed live in the think2 journal:

### 1. Multiple services mutate the source checkout

`/srv/src/vibe-kanban` is simultaneously:

- the **deploy artifact store** — every service ExecStarts from inside it
  (`node …/npx-cli/bin/cli.js`, `…/npx-cli/dist/linux-x64/remote`,
  `…/dist/linux-x64/relay-server`); the CLI launcher *unlinks and re-extracts*
  binaries into `npx-cli/dist/<platform>/` on every service start, so service
  users need write access to the repo;
- the **build input** — the rebuild service publishes fresh `dist/` into it
  under flock;
- a **git workspace shared with the app** — vibe-kanban manages this repo as a
  project, so task worktrees (owned by `vibe-kanban-dev`) register under
  `/srv/src/vibe-kanban/.git/worktrees/`;
- a **poll target** — `git-projects-update` hard-resets it every 15 minutes.

Observed failure (Jul 05 23:42): the rebuild's `git worktree prune` (running as
`developer`) hit `failed to delete '.git/worktrees/vibe-kanban1': Permission
denied` on the app's foreign-owned worktree registrations. Any actor that
wipes/resets `npx-cli/dist` takes the running services' binaries with it.

### 2. Build failures back up silently

- Jul 05 23:35: rebuild exited `status=127` one second in; **no retry, no
  alert** — deploy stalled until the next upstream commit happened to re-fire
  the stamp.
- Jul 05 22:39: rebuild killed mid-run (`status=15/TERM`, result `signal`) —
  same silence.
- The trigger is **edge-triggered** (PathChanged on the stamp): once a build
  for SHA *N* fails, later polls reset to the same SHA without rewriting the
  stamp, so nothing re-fires.
- The self-heal guard checks only artifact **existence**, not **freshness**:
  binaries from commit *N−1* are "present", so a stalled deploy of commit *N*
  is invisible to it.
- No notification path exists for any of this; failures are discovered only
  when someone notices the running version is stale.

## Goal

A deployment pipeline where:

1. **Services never run from, or write to, the source checkout.** Builds
   publish immutable, versioned releases outside the repo; services ExecStart
   from a `current` symlink that is flipped atomically.
2. **Deploys are level-triggered (reconciled), not just edge-triggered.** The
   system converges on "deployed SHA == stamped SHA" even after crashes,
   SIGTERM kills, or exit-127s, without waiting for the next upstream commit.
3. **Failures are loud.** Any rebuild failure, health-check failure, or
   persistent deploy drift publishes an ntfy alert with journal context.
4. **Restarts are health-gated with rollback.** After flipping `current` and
   restarting, the deployer probes each service's existing `/health` endpoint;
   on failure it flips back to the previous release, restarts, and alerts.

## Non-goals

- No change to the public npm/npx distribution path (`ensureBinary` download
  flow, `npm-cdn.vibekanban.com`); this is about the self-hosted local-build
  deployment only.
- No Docker/Kubernetes migration — the strategy stays NixOS units + systemd,
  matching the rest of the homelab.
- No change to how the app itself creates task worktrees (`.git/worktrees/`
  stays multi-writer by design); the deploy machinery must simply tolerate it.
- No blue/green for the SQLite-backed local instance (single writer); rollback
  is symlink-flip + restart, not parallel instances.

## Design

Changes span both repos in this workspace.

### A. vibe-kanban repo: versioned release publishing in `local-build.sh`

Extend the existing `VK_REMOTE_STATIC_RELEASES` pattern (versioned
`build-<id>/` trees + atomic `current` symlink + prune) to the **binaries**:

- New env `VK_RELEASES_DIR` (set by the deploy host, absent for CI/dev — same
  gating style as `VK_REMOTE_STATIC_RELEASES`). When set, after all binaries
  build, `local-build.sh` publishes:

  ```
  $VK_RELEASES_DIR/
    build-<BUILD_ID>/
      release.json          # {"sha": "<git sha>", "build_id": "...", "built_at": "..."}
      bin/
        vibe-kanban         # extracted server binary
        vibe-kanban-mcp
        vibe-kanban-review
        remote
        relay-server
    current  -> build-<BUILD_ID>     # atomic rename flip
    previous -> build-<old>          # previous current, for rollback
  ```

- Binaries are published **extracted** (no zips) so services exec them
  directly — no launcher, no runtime extraction, no writes at service start.
- `sha` comes from the build tree's `git rev-parse HEAD` (the rebuild service
  builds a pinned worktree, so this equals the stamp SHA).
- Flip protocol: stage `.current-<BUILD_ID>` symlink, `mv -Tf` over `current`
  (atomic rename, same as remote-web); repoint `previous` to the old target
  first. Prune to the 3 newest `build-*` dirs, never deleting the targets of
  `current`/`previous`.
- The existing `npx-cli/dist` staging/publish continues unchanged (CI and
  local dev still use it); on the deploy host it simply stops being what
  services run from.

### B. homelab repo: `modules/vibe-kanban-rebuild.nix` becomes a deployer

1. **Publish to releases, not the checkout.** Set `VK_RELEASES_DIR` (new
   option `releasesDir`, default `/srv/vk-releases`; tmpfiles-managed like
   `staticReleasesDir`). Drop the step that publishes `npx-cli/dist` +
   `cli.js` back into `/srv/src/vibe-kanban` — the checkout becomes build
   input only.
2. **Health-gated restart with rollback.** Replace the blind
   `ExecStartPost systemctl restart …` with a deploy step in the script:
   restart units, then poll each service's `/health`
   (dev `127.0.0.1:3334/api/health`, remote `127.0.0.1:8082/health`, relay
   `127.0.0.1:8083/health`) with a bounded retry window (~60s). On failure:
   flip `current` back to `previous`, restart again, and exit non-zero so the
   failure alert fires.
3. **Failure alerting.** `OnFailure=vibe-kanban-deploy-alert.service` on the
   rebuild unit — a oneshot that posts the failing unit + last ~30 journal
   lines to ntfy (`https://ntfy.vasandani.dev/<topic>`, topic a module option;
   same capability-URL pattern as the comin watchdog).
4. **Reconciler guard (freshness, not just existence).** Extend
   `vibe-kanban-rebuild-guard` to also compare the stamp SHA against
   `releases/current/release.json`'s `sha`. If they differ and no rebuild is
   active/queued, start one. Persist a consecutive-failure counter in a state
   file; at ≥2 consecutive reconcile attempts without convergence, fire the
   alert unit (deploy stalled — the "silent backlog" now pages). Keep the
   existing remote-web serve-path check; existence checks now target
   `releases/current/bin/*`.
5. **Fix the worktree collision.** Remove the global `git worktree prune`
   (it trips over the app's foreign-owned registrations). Clean up only our
   own build tree: `git worktree remove --force` it, and remove its specific
   admin dir `.git/worktrees/build-tree` if left behind; both are created by
   the build user, so no permission conflicts.

### C. homelab repo: services exec from the release

- `vibe-kanban.nix` (dev instance): new option `releasesDir`; when set,
  `ExecStart` runs `${releasesDir}/current/bin/vibe-kanban` directly (no
  node/cli.js, no `VIBE_KANBAN_LOCAL`), and the unit environment sets
  `VIBE_KANBAN_MCP_COMMAND=${releasesDir}/current/bin/vibe-kanban-mcp` so
  launched agents get the co-built MCP binary (per
  `vibe-kanban.env.example`'s documented override). The `developers`
  group-write requirement for the *service user* (needed only for the cli.js
  extract step) is retained for now because agents still build in worktrees,
  but the service itself no longer writes to the repo.
- `vibe-kanban-remote.nix` / `vibe-kanban-relay.nix`: default `binaryPath` to
  `${releasesDir}/current/bin/{remote,relay-server}`; drop the
  `ExecStartPre chmod +x` (release binaries are published executable).
- `vibe-kanban-mcp.nix`: point the gateway at
  `${releasesDir}/current/bin/vibe-kanban-mcp` instead of `cli.js`.
- `hosts/think/think2.nix`: enable the new options.
- Services keep running the **old** release until the deployer flips
  `current`; a failed build can no longer leave them binary-less (today's
  crash-loop-on-wiped-dist mode disappears structurally).

### D. Migration / compatibility

- First deploy after the switch: guard sees no `releases/current` →
  triggers a rebuild → release published → units (now pointing at
  `current`) restart healthy. Until that first successful build, units may
  fail to start; ordering the nixos switch after a manual
  `systemctl start vibe-kanban-rebuild` avoids a gap, and the guard
  self-heals regardless.
- `expectedArtifacts` option semantics change (release paths); think2.nix
  doesn't override it, so no host churn.
- Rollback runbook: `ln -sfn <build-dir> /srv/vk-releases/.roll && mv -Tf
  /srv/vk-releases/.roll /srv/vk-releases/current && systemctl restart
  vibe-kanban-dev vibe-kanban-remote vibe-kanban-relay`.

## Acceptance criteria

1. No systemd unit ExecStarts from, or writes at startup into,
   `/srv/src/vibe-kanban`.
2. `local-build.sh` with `VK_RELEASES_DIR` set publishes an extracted,
   `release.json`-stamped, atomically-flipped release; without it, behavior
   is byte-identical to today (CI unaffected).
3. A rebuild that fails (any exit path: compile error, 127, SIGTERM) leaves
   `current` untouched, services running, and posts an ntfy alert.
4. A deploy that stalls (stamp SHA ≠ deployed SHA across two guard runs)
   posts an ntfy alert and keeps retrying the build.
5. A release whose services fail their `/health` probes is rolled back to
   `previous` automatically, with an alert.
6. The rebuild never runs `git worktree prune` against the shared checkout;
   app-owned worktree registrations cannot fail the build.
7. `nix flake check` passes for the homelab repo; `bash -n local-build.sh`
   and a dry local build pass for vibe-kanban.

## Risks

- **Hardcoded health ports** in the deployer must match think2's service
  config; expose them as module options with think2's values as defaults.
- **`previous` pointing at a pruned dir** — prune must exclude both symlink
  targets (spec'd above).
- **First-boot ordering** (no release yet) — covered by guard self-heal;
  documented in the runbook.
- **ntfy topic in a public-ish repo** — follows existing repo practice
  (comin watchdog commits its topic); topic is unguessable, not a secret
  credential.
