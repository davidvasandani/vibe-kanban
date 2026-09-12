#!/usr/bin/env bash
set -euo pipefail

# Build and deploy one pinned Vibe Kanban commit without sharing the regular
# rebuild service's Cargo target directory. The build may atomically publish a
# new `current`; this wrapper snapshots the known-good release first and owns
# the restart, health gate, and rollback that follow.

log() {
  printf 'vibe-kanban-cluster-rollout: %s\n' "$*"
}

die() {
  log "ERROR: $*"
  exit 1
}

require_absolute() {
  case "$2" in
    /*) ;;
    *) die "$1 must be an absolute path (got: $2)" ;;
  esac
}

probe() {
  local name=$1 url=$2 deadline
  deadline=$((SECONDS + VK_ROLLOUT_HEALTH_TIMEOUT_SECONDS))
  until "$VK_ROLLOUT_CURL" -fsS -m 5 -o /dev/null "$url"; do
    if (( SECONDS >= deadline )); then
      log "health check FAILED for $name ($url)"
      return 1
    fi
    sleep "$VK_ROLLOUT_HEALTH_RETRY_SECONDS"
  done
  log "health OK for $name"
}

atomic_link() {
  local target=$1 link=$2 staged
  staged="$(dirname "$link")/.rollout-link-$$-$(basename "$link")"
  ln -sfn "$target" "$staged"
  mv -Tf "$staged" "$link"
}

restore_known_good_links() {
  atomic_link "$known_good_release" "$VK_RELEASES_DIR/current"
}

rollback() {
  local reason=$1
  log "unhealthy after deploy: $reason"
  if [[ -z "$known_good_release" || ! -d "$known_good_release" ]]; then
    die "known-good release is unavailable; refusing an unverified rollback"
  fi

  log "rolling back current -> $known_good_release"
  restore_known_good_links
  "$VK_ROLLOUT_SYSTEMCTL" restart $VK_ROLLOUT_UNITS || true

  local rollback_failed=""
  while IFS='|' read -r name url; do
    [[ -z "$name" ]] && continue
    probe "$name rollback" "$url" || rollback_failed="$rollback_failed $name"
  done <<< "$VK_ROLLOUT_HEALTH_CHECKS"
  [[ -z "$rollback_failed" ]] \
    || die "rollback was attempted but remained unhealthy:$rollback_failed"
  die "candidate failed health checks and was rolled back: $reason"
}

VK_ROLLOUT_REPO=${VK_ROLLOUT_REPO:-/srv/src/vibe-kanban}
VK_ROLLOUT_BUILD_TREE=${VK_ROLLOUT_BUILD_TREE:-/srv/src/vibe-kanban-rebuild-cache/cluster-rollout-tree}
VK_ROLLOUT_TARGET_DIR=${VK_ROLLOUT_TARGET_DIR:-/srv/src/vibe-kanban-cluster-rollout-cache/target}
VK_RELEASES_DIR=${VK_RELEASES_DIR:-/srv/vk-releases}
VK_ROLLOUT_SYSTEMCTL=${VK_ROLLOUT_SYSTEMCTL:-systemctl}
VK_ROLLOUT_CURL=${VK_ROLLOUT_CURL:-curl}
VK_ROLLOUT_UNITS=${VK_ROLLOUT_UNITS:-"vibe-kanban-dev.service vibe-kanban-remote.service vibe-kanban-relay.service"}
VK_ROLLOUT_HEALTH_CHECKS=${VK_ROLLOUT_HEALTH_CHECKS:-$'vibe-kanban-dev.service|http://127.0.0.1:3334/api/health\nvibe-kanban-remote.service|http://127.0.0.1:8082/health\nvibe-kanban-relay.service|http://127.0.0.1:8083/health'}
VK_ROLLOUT_HEALTH_TIMEOUT_SECONDS=${VK_ROLLOUT_HEALTH_TIMEOUT_SECONDS:-60}
VK_ROLLOUT_HEALTH_RETRY_SECONDS=${VK_ROLLOUT_HEALTH_RETRY_SECONDS:-2}
VK_ROLLOUT_BUILD_COMMAND=${VK_ROLLOUT_BUILD_COMMAND:-"pnpm install --frozen-lockfile && bash ./local-build.sh"}
intended_sha=${1:-${VK_ROLLOUT_SHA:-}}

[[ "$intended_sha" =~ ^[0-9a-f]{40}$ ]] || die "a full 40-character commit SHA is required"
for path_var in VK_ROLLOUT_REPO VK_ROLLOUT_BUILD_TREE VK_ROLLOUT_TARGET_DIR VK_RELEASES_DIR; do
  require_absolute "$path_var" "${!path_var}"
done
[[ "$VK_ROLLOUT_TARGET_DIR" != /srv/src/vibe-kanban-rebuild-cache/target ]] \
  || die "cluster rollout must not share the regular rebuild target directory"
[[ "$VK_ROLLOUT_BUILD_TREE" != "$VK_ROLLOUT_REPO" && "$VK_ROLLOUT_BUILD_TREE" != / ]] \
  || die "build worktree must be separate from the source repository"

repo_sha=$(git -C "$VK_ROLLOUT_REPO" rev-parse "$intended_sha^{commit}")
[[ "$repo_sha" == "$intended_sha" ]] || die "intended commit is not available in $VK_ROLLOUT_REPO"

known_good_release=$(readlink -f "$VK_RELEASES_DIR/current" 2>/dev/null || true)
[[ -n "$known_good_release" && -f "$known_good_release/release.json" ]] \
  || die "current does not resolve to a self-describing known-good release"
known_good_sha=$(jq -r '.sha // empty' "$known_good_release/release.json")
[[ "$known_good_sha" =~ ^[0-9a-f]{40}$ ]] \
  || die "known-good release has an invalid release.json SHA"

# Do not begin a deployment from an already-broken baseline.
while IFS='|' read -r name url; do
  [[ -z "$name" ]] && continue
  probe "$name preflight" "$url" || die "preflight health check failed for $name"
done <<< "$VK_ROLLOUT_HEALTH_CHECKS"

log "building pinned commit $intended_sha"
git -C "$VK_ROLLOUT_REPO" worktree remove --force "$VK_ROLLOUT_BUILD_TREE" 2>/dev/null || true
rm -rf "$VK_ROLLOUT_BUILD_TREE"
git -C "$VK_ROLLOUT_REPO" worktree add --force --detach "$VK_ROLLOUT_BUILD_TREE" "$intended_sha"

mkdir -p "$VK_ROLLOUT_TARGET_DIR"
if ! (
  cd "$VK_ROLLOUT_BUILD_TREE"
  [[ "$(git rev-parse HEAD)" == "$intended_sha" ]] \
    || die "build worktree does not match intended commit"
  export CARGO_TARGET_DIR="$VK_ROLLOUT_TARGET_DIR"
  export CARGO_INCREMENTAL=0
  export VK_RELEASES_DIR
  export VK_RELEASES_DEFER_FLIP=1
  bash -c "$VK_ROLLOUT_BUILD_COMMAND"
); then
  restore_known_good_links
  die "candidate build or publish failed; known-good links restored"
fi

candidate=$(
  find "$VK_RELEASES_DIR" -mindepth 2 -maxdepth 2 -type f -name release.json -printf '%T@ %h\n' \
    | sort -nr \
    | cut -d' ' -f2- \
    | while IFS= read -r release; do
        [[ "$(jq -r '.sha // empty' "$release/release.json" 2>/dev/null || true)" == "$intended_sha" ]] \
          && { printf '%s\n' "$release"; break; }
      done
)
if [[ -z "$candidate" || "$candidate" == "$known_good_release" ]]; then
  restore_known_good_links
  die "build did not stage a distinct candidate for $intended_sha"
fi

for artifact in vibe-kanban vibe-kanban-mcp vibe-kanban-review remote relay-server vibe-kanban-worker; do
  [[ -f "$candidate/bin/$artifact" && -x "$candidate/bin/$artifact" ]] || {
    restore_known_good_links
    die "candidate is missing executable artifact: $artifact"
  }
done
log "verified candidate release $candidate for $intended_sha"

# Only verified, complete artifacts reach the live symlinks.
atomic_link "$known_good_release" "$VK_RELEASES_DIR/previous"
atomic_link "$candidate" "$VK_RELEASES_DIR/current"
log "candidate flipped live; starting health gate"

failed=""
if ! "$VK_ROLLOUT_SYSTEMCTL" restart $VK_ROLLOUT_UNITS; then
  failed="restart"
fi
if [[ -z "$failed" ]]; then
  while IFS='|' read -r name url; do
    [[ -z "$name" ]] && continue
    probe "$name" "$url" || failed="$failed $name"
  done <<< "$VK_ROLLOUT_HEALTH_CHECKS"
fi
[[ -z "$failed" ]] || rollback "$failed"

log "deploy complete and healthy for $intended_sha"
