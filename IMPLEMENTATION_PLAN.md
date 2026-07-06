# Implementation Plan: Production-Grade Deployment Strategy

See `SPEC.md` for the full design and observed failure evidence. Changes span
both workspace repos: `vibe-kanban/` (build script + deploy docs) and
`homelab/` (NixOS modules for think2).

## Step 1 — `local-build.sh`: versioned binary releases (vibe-kanban repo)

Gated on new env `VK_RELEASES_DIR` (absent → byte-identical behavior for CI
and local dev), mirroring the existing `VK_REMOTE_STATIC_RELEASES` block:

- After the atomic `npx-cli/dist` publish, when `VK_RELEASES_DIR` is set:
  - Create `$VK_RELEASES_DIR/build-${BUILD_ID}/bin/` and copy the five
    binaries **extracted** from `$CARGO_TARGET_DIR/release/`:
    `server → vibe-kanban`, `vibe-kanban-mcp`, `review → vibe-kanban-review`,
    `remote`, `relay-server`; `chmod 755` them.
  - Write `build-${BUILD_ID}/release.json` with `sha` (`git rev-parse HEAD`),
    `build_id`, `built_at` (ISO-8601 UTC).
  - Repoint `previous` at the old `current` target (if any), then atomically
    flip `current` (stage `.current-${BUILD_ID}` symlink, `mv -Tf`).
  - Prune to the 3 newest `build-*` dirs, always keeping the resolved
    targets of `current` and `previous`.
  - `chmod -R g+rwX,o+rX` the release so non-builder service users can exec.
- Validate with `bash -n local-build.sh` and a scratch-dir dry run of the
  publish function (no full rebuild required for the publish logic itself).

## Step 2 — `vibe-kanban-rebuild.nix`: publish → releases, health-gate, alert (homelab)

1. New options: `releasesDir` (default `/srv/vk-releases`), `healthChecks`
   (list of `{unit, url}` with think2 defaults), `ntfyTopicUrl`
   (nullable; when set, failures page).
2. tmpfiles rule for `releasesDir` (`2775 developer developers`, like
   `staticReleasesDir`).
3. Script changes:
   - Export `VK_RELEASES_DIR=${releasesDir}`.
   - **Remove** the publish-back of `npx-cli/dist` + `cli.js` into the
     checkout (and its flock section).
   - **Remove** `git worktree prune`; clean only our own build tree
     (`worktree remove --force` + targeted `rm -rf` of the tree and its
     `.git/worktrees/build-tree` admin dir).
   - After `local-build.sh` succeeds: `systemctl restart` the units (moved
     from `ExecStartPost` into the script), then poll each health URL with
     curl, ~60s budget. On any probe failing: flip `current` back to
     `previous`, restart units again, log loudly, `exit 1`.
4. `OnFailure=vibe-kanban-deploy-alert.service` on the rebuild unit; new
   oneshot alert unit posts unit name + `journalctl -u vibe-kanban-rebuild
   -n 30` tail to the ntfy topic (skip silently when topic unset).
5. Guard → reconciler:
   - Missing-artifact check now targets `${releasesDir}/current/bin/*`
     (update `expectedArtifacts` default) + keep the remote-web serve check.
   - New freshness check: `stamp SHA != release.json sha` (jq) while
     `vibe-kanban-rebuild.service` is inactive → `systemctl start --no-block`
     it.
   - Consecutive-failure counter in `${releasesDir}/.reconcile-failures`;
     at ≥2, also fire the alert unit ("deploy stalled at <sha>").
6. Polkit rule already covers `restartUnits`; the rollback path restarts the
   same units, so no polkit change.

## Step 3 — services exec from the release (homelab)

- `vibe-kanban.nix` (`services.vibe-kanban-dev`): new nullable option
  `releasesDir`. When set: `ExecStart=${releasesDir}/current/bin/vibe-kanban`
  (drop node/cli.js + `VIBE_KANBAN_LOCAL`), add unit env
  `VIBE_KANBAN_MCP_COMMAND=${releasesDir}/current/bin/vibe-kanban-mcp`.
  Both the token-script and plain-ExecStart branches must honor it.
- `vibe-kanban-remote.nix` / `vibe-kanban-relay.nix`: `binaryPath` defaults
  change to `/srv/vk-releases/current/bin/{remote,relay-server}` (think2
  stays on defaults); drop `ExecStartPre chmod +x`.
- `vibe-kanban-mcp.nix`: exec `…/current/bin/vibe-kanban-mcp` instead of
  `cli.js --mcp`.
- `hosts/think/think2.nix`: set `releasesDir`/topic options; update comments
  contradicted by the change (e.g. cli.js extraction rationale for the
  service user's `developers` membership).

## Step 4 — migration + docs

- Runbook: first-deploy ordering, manual rollback one-liner, alert topic —
  in module header comments + a short `homelab/docs/` note if warranted.
- vibe-kanban repo: mention `VK_RELEASES_DIR` in `vibe-kanban.env.example` /
  `vibe-kanban.service.example` where they describe installing binaries.

## Step 5 — verify

1. `bash -n vibe-kanban/local-build.sh`; scratch-dir dry-run of the release
   publish with a fake `CARGO_TARGET_DIR`.
2. `nix flake check` (or targeted `nix eval` of the think2 config) in
   `homelab/`.
3. Quoting / `set -euo pipefail` review of embedded module scripts.
4. `pnpm run format` in vibe-kanban if any TS/Rust touched (expected: none).

## Step 6 — review, docs, knowledge, PRs

- Codex review of both diffs; fix confirmed findings.
- Docs stage: wiki page for the deployment architecture + INDEX update.
- PRs: one against `davidvasandani/vibe-kanban` main, one against
  `davidvasandani/homelab` main.
