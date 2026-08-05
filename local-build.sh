#!/bin/bash

set -e  # Exit on any error

# Make every file we create group-writable. Two builders (CI runner +
# local rebuild service) can both produce artifacts on the same host;
# keeping them group-writable means whoever runs next can overwrite
# what the previous run left behind, rather than EPERM'ing on chmod.
umask 002

# Per-process staging suffix so two concurrent builds don't clobber each
# other's .dist-staging tree. The publish step (see end of file) takes a
# small flock around the rename into place, so only the actual swap is
# serialized — both builds can compile in parallel against their own
# CARGO_TARGET_DIRs.
BUILD_ID="$$-$(date +%s)"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

# Map architecture names
case "$ARCH" in
  x86_64)
    ARCH="x64"
    ;;
  arm64|aarch64)
    ARCH="arm64"
    ;;
  *)
    echo "⚠️  Warning: Unknown architecture $ARCH, using as-is"
    ;;
esac

# Map OS names
case "$OS" in
  linux)
    OS="linux"
    ;;
  darwin)
    OS="macos"
    ;;
  *)
    echo "⚠️  Warning: Unknown OS $OS, using as-is"
    ;;
esac

PLATFORM="${OS}-${ARCH}"

# Set CARGO_TARGET_DIR if not defined
if [ -z "$CARGO_TARGET_DIR" ]; then
  CARGO_TARGET_DIR="target"
fi

echo "🔍 Detected platform: $PLATFORM"
echo "🔧 Using target directory: $CARGO_TARGET_DIR"

# Set API base URL for remote features
export VK_SHARED_API_BASE="https://api.vibekanban.com"
export VITE_VK_SHARED_API_BASE="https://api.vibekanban.com"

echo "🧹 Cleaning previous builds..."
# Build artifacts into a staging dir and swap it into place only after every
# binary has been produced (see the publish step at the end of this script).
# This guarantees the live npx-cli/dist -- which a running vibe-kanban or
# remote service may be serving from -- is never left empty by a build that
# fails partway. Previously "rm -rf npx-cli/dist" ran up front, so any later
# failure (web build, cargo, ...) wiped the running deployment and left the
# service crash-looping on a missing binary.
DIST_STAGING="npx-cli/.dist-staging-${BUILD_ID}"
rm -rf "$DIST_STAGING"
mkdir -p "$DIST_STAGING/$PLATFORM"
# Always clean up our staging dir on exit, even if the build aborts.
# Without this, concurrent failed builds would leave .dist-staging-* trees
# behind and slowly fill the worktree.
trap 'rm -rf "$DIST_STAGING" 2>/dev/null || true' EXIT

echo "🔨 Building web app..."
(cd packages/local-web && npm run build)

# Build the remote deployment frontend in the SAME script that compiles the
# `remote` binary, so the served UI and the backend are always from one
# commit. The remote binary embeds the git sha at compile time (shown in the
# app's corner); previously nothing here built packages/remote-web, so that
# sha advanced every deploy while the served frontend stayed frozen. Always
# building it (not just on the deploy host) also means CI/local builds catch
# remote-web breakage instead of shipping a stale UI.
echo "🔨 Building remote web app..."
(cd packages/remote-web && npm run build)

echo "🔨 Building Rust binaries..."
# Build only the CLI binaries. Building the whole workspace pulls in
# crates/tauri-app, whose GTK/glib system deps aren't installed on the
# headless Linux CI runner. The Tauri build is opt-in below via --desktop.
cargo build --release --manifest-path Cargo.toml \
  --bin server --bin vibe-kanban-mcp --bin review --bin vibe-kanban-worker

echo "Building Remote API binary..."
# crates/remote is excluded from the cargo workspace (exclude = [...]), so it
# has to be built via its own manifest. --target-dir keeps its output next to
# the workspace bins regardless of whether CARGO_TARGET_DIR is exported.
cargo build --release --manifest-path crates/remote/Cargo.toml \
  --target-dir "$CARGO_TARGET_DIR" --bin remote

echo "Building Relay tunnel server binary..."
# crates/relay-tunnel is also excluded from the workspace (exclude = [...]), so
# it builds via its own manifest. This is the Remote Access relay broker that
# hosts register with and clients tunnel through.
cargo build --release --manifest-path crates/relay-tunnel/Cargo.toml \
  --target-dir "$CARGO_TARGET_DIR" --bin relay-server

echo "📦 Creating distribution package..."

# Copy the main binary
cp ${CARGO_TARGET_DIR}/release/server vibe-kanban
zip -q vibe-kanban.zip vibe-kanban
rm -f vibe-kanban 
mv vibe-kanban.zip "$DIST_STAGING/$PLATFORM/vibe-kanban.zip"

# Copy the MCP binary
cp ${CARGO_TARGET_DIR}/release/vibe-kanban-mcp vibe-kanban-mcp
zip -q vibe-kanban-mcp.zip vibe-kanban-mcp
rm -f vibe-kanban-mcp
mv vibe-kanban-mcp.zip "$DIST_STAGING/$PLATFORM/vibe-kanban-mcp.zip"

# Copy the Review CLI binary
cp ${CARGO_TARGET_DIR}/release/review vibe-kanban-review
zip -q vibe-kanban-review.zip vibe-kanban-review
rm -f vibe-kanban-review
mv vibe-kanban-review.zip "$DIST_STAGING/$PLATFORM/vibe-kanban-review.zip"

# Copy the Remote API binary (self-hosted OAuth + team management backend).
# It is run directly by the remote service, so it ships unzipped + executable.
cp ${CARGO_TARGET_DIR}/release/remote "$DIST_STAGING/$PLATFORM/remote"
chmod +x "$DIST_STAGING/$PLATFORM/remote"

# Copy the Relay tunnel server binary (Remote Access broker).
# Run directly by the relay service, so it ships unzipped + executable.
cp ${CARGO_TARGET_DIR}/release/relay-server "$DIST_STAGING/$PLATFORM/relay-server"
chmod +x "$DIST_STAGING/$PLATFORM/relay-server"

# Cluster worker daemon, deployed directly by the homelab worker units.
cp ${CARGO_TARGET_DIR}/release/vibe-kanban-worker "$DIST_STAGING/$PLATFORM/vibe-kanban-worker"
chmod +x "$DIST_STAGING/$PLATFORM/vibe-kanban-worker"

echo "✅ CLI build complete!"
echo "📁 Files created:"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban.zip"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban-mcp.zip"
echo "   - npx-cli/dist/$PLATFORM/vibe-kanban-review.zip"

# Optionally build the Tauri desktop app
if [[ "$1" == "--desktop" || "$1" == "--all" ]]; then
  # Map to Tauri platform naming
  case "$OS" in
    macos) TAURI_OS="darwin" ;;
    linux) TAURI_OS="linux" ;;
    *) TAURI_OS="$OS" ;;
  esac
  case "$ARCH" in
    arm64) TAURI_ARCH="aarch64" ;;
    x64) TAURI_ARCH="x86_64" ;;
    *) TAURI_ARCH="$ARCH" ;;
  esac
  TAURI_PLATFORM="${TAURI_OS}-${TAURI_ARCH}"

  echo ""
  echo "🖥️  Building Tauri desktop app for $TAURI_PLATFORM..."

  # Replace the updater endpoint placeholder with a dummy URL for local builds
  # (CI injects the real R2 URL; locally the updater is non-functional)
  TAURI_CONF="crates/tauri-app/tauri.conf.json"
  node -e "
    const fs = require('fs');
    const conf = JSON.parse(fs.readFileSync('$TAURI_CONF', 'utf8'));
    conf.plugins.updater.endpoints = conf.plugins.updater.endpoints.map(e =>
      e === '__TAURI_UPDATE_ENDPOINT__' ? 'https://localhost/disabled' : e
    );
    fs.writeFileSync('$TAURI_CONF', JSON.stringify(conf, null, 2) + '\n');
  "

  cargo tauri build

  # Restore tauri.conf.json
  git checkout -- "$TAURI_CONF"

  TAURI_DIST="$DIST_STAGING/tauri/$TAURI_PLATFORM"
  mkdir -p "$TAURI_DIST"

  BUNDLE_DIR="${CARGO_TARGET_DIR}/release/bundle"
  # Copy updater artifacts (tar.gz bundles or NSIS exe)
  find "$BUNDLE_DIR" -name "*.app.tar.gz" ! -name "*.sig" -exec cp {} "$TAURI_DIST/" \; 2>/dev/null || true
  find "$BUNDLE_DIR" -name "*.AppImage.tar.gz" ! -name "*.sig" -exec cp {} "$TAURI_DIST/" \; 2>/dev/null || true
  find "$BUNDLE_DIR" -name "*-setup.exe" -exec cp {} "$TAURI_DIST/" \; 2>/dev/null || true

  echo "✅ Desktop app built:"
  ls -la "$TAURI_DIST/"
fi

echo ""
echo "📦 Installing npx-cli dependencies..."
(cd npx-cli && npm ci)

echo ""
echo "🔨 Building npx-cli TypeScript..."
(cd npx-cli && npm run build)

# --- Atomic publish -------------------------------------------------------
# Only now that every binary built successfully do we replace the live dist.
# Any failure above aborts the script (set -e) leaving the previous, working
# npx-cli/dist untouched, so a running service keeps serving.
#
# A small flock around just the rename serializes the publish step across
# concurrent builders (CI + local rebuild). Each builder has its own
# .dist-staging-${BUILD_ID} from above, so only this critical section needs
# coordination. The lock file lives next to npx-cli/dist so it shares the
# same filesystem (flock needs a real local file).
echo "Publishing build to npx-cli/dist..."
(
  flock --exclusive 200
  rm -rf npx-cli/dist
  mv "$DIST_STAGING" npx-cli/dist
) 200>"npx-cli/.publish-lock"

# Make the published artifacts readable/traversable by the non-builder service
# users (vibe-kanban-dev, vibe-kanban-remote) regardless of the build umask.
# g+rwX so a future builder running as a different user (but same group) can
# overwrite this tree without EPERM'ing on chmod. Scoped to dist only;
# node_modules is intentionally excluded because it can contain files owned
# by another user from a prior build, which would abort under set -e.
chmod -R g+rwX,o+rX npx-cli/dist || true

# --- Publish versioned binary release ---------------------------------------
# When VK_RELEASES_DIR is set (deploy hosts only — CI and local dev leave it
# unset and are unaffected), publish the freshly-built binaries as an
# immutable, versioned release that services exec directly:
#
#   $VK_RELEASES_DIR/
#     build-<id>/bin/{vibe-kanban,vibe-kanban-mcp,vibe-kanban-review,
#                     remote,relay-server,vibe-kanban-worker}
#     build-<id>/release.json      {"sha", "build_id", "built_at"}
#     current  -> build-<id>       (atomic rename flip)
#     previous -> old current      (single-step rollback target)
#
# This extends the VK_REMOTE_STATIC_RELEASES pattern (see the remote-web
# publish below) to the binaries.
# Deployed services stop running out of npx-cli/dist inside the source
# checkout — where a repo reset/chmod/wipe can take the running deployment
# down — and instead exec extracted binaries behind a `current` symlink.
#
# The publish is split in two around the remote-web publish below:
# STAGING (here) happens before the remote-web `current` flips, the binary
# `current` FLIP happens after it. Any failure therefore leaves the deployed
# system fully consistent — either nothing flipped, or (in the few-symlink-op
# window between the two flips) a version drift the deploy reconciler detects
# and retries. `current` always resolves to a release whose every binary
# built.
if [ -n "${VK_RELEASES_DIR:-}" ]; then
  # Symlink targets are resolved relative to the symlink's own directory, so
  # a relative VK_RELEASES_DIR would produce a self-prefixed, dangling
  # `current`. Canonicalize once; everything below uses the absolute path.
  VK_RELEASES_DIR="$(mkdir -p "$VK_RELEASES_DIR" && cd "$VK_RELEASES_DIR" && pwd)"
  echo "📦 Staging binary release for ${VK_RELEASES_DIR}..."
  RELEASE="${VK_RELEASES_DIR}/build-${BUILD_ID}"
  rm -rf "$RELEASE"
  mkdir -p "$RELEASE/bin"
  # Deploy names: `server` ships as `vibe-kanban` and `review` as
  # `vibe-kanban-review`, matching the names the npm CLI extracts.
  cp "${CARGO_TARGET_DIR}/release/server" "$RELEASE/bin/vibe-kanban"
  cp "${CARGO_TARGET_DIR}/release/vibe-kanban-mcp" "$RELEASE/bin/vibe-kanban-mcp"
  cp "${CARGO_TARGET_DIR}/release/review" "$RELEASE/bin/vibe-kanban-review"
  cp "${CARGO_TARGET_DIR}/release/remote" "$RELEASE/bin/remote"
  cp "${CARGO_TARGET_DIR}/release/relay-server" "$RELEASE/bin/relay-server"
  cp "${CARGO_TARGET_DIR}/release/vibe-kanban-worker" "$RELEASE/bin/vibe-kanban-worker"
  chmod 755 "$RELEASE/bin/"*

  # Self-describing release: a deploy reconciler compares `sha` against its
  # desired-revision stamp to detect stalled deploys mechanically.
  cat > "$RELEASE/release.json" <<EOF
{
  "sha": "$(git rev-parse HEAD)",
  "build_id": "${BUILD_ID}",
  "built_at": "$(date -u +%FT%TZ)"
}
EOF

  # Group-writable for whichever builder runs next; world-readable/executable
  # for the (non-builder) service users, regardless of the build umask.
  chmod -R g+rwX,o+rX "$RELEASE" || true
fi

# --- Publish remote web frontend ------------------------------------------
# The remote service (crates/remote) serves the UI from a fixed path via
# ServeDir, where that path is a symlink the deploy host points at the
# "current" release below. Publishing here — from the same build that just
# compiled the `remote` binary — is what guarantees the served frontend and
# the backend binary are always the same commit.
#
# Gated on VK_REMOTE_STATIC_RELEASES (set by the deploy host) so CI and local
# developer builds, which have no such directory, are unaffected. The builder
# only writes inside this releases dir, so it needs no privileges on the
# served symlink's parent.
if [ -n "${VK_REMOTE_STATIC_RELEASES:-}" ]; then
  echo "📦 Publishing remote web to ${VK_REMOTE_STATIC_RELEASES}..."
  REMOTE_RELEASE="${VK_REMOTE_STATIC_RELEASES}/build-${BUILD_ID}"
  rm -rf "$REMOTE_RELEASE"
  mkdir -p "$REMOTE_RELEASE"
  cp -a packages/remote-web/dist/. "$REMOTE_RELEASE/"
  # World-readable so the (non-builder) remote service user can serve it
  # regardless of the build umask.
  chmod -R g+rwX,o+rX "$REMOTE_RELEASE" || true
  if [ "${VK_RELEASES_DEFER_FLIP:-0}" = "1" ]; then
    echo "✅ Remote web staged (live flip deferred): $REMOTE_RELEASE"
  else
    # Atomic repoint: stage a new "current" symlink, then rename it over the
    # old one. rename(2) is atomic, so the live site never resolves to a
    # half-copied tree.
    ln -sfn "$REMOTE_RELEASE" "${VK_REMOTE_STATIC_RELEASES}/.current-${BUILD_ID}"
    mv -Tf "${VK_REMOTE_STATIC_RELEASES}/.current-${BUILD_ID}" \
      "${VK_REMOTE_STATIC_RELEASES}/current"
    # Retain the 3 most recent builds; prune older ones so the releases dir
    # doesn't grow without bound.
    ls -1dt "${VK_REMOTE_STATIC_RELEASES}"/build-* 2>/dev/null \
      | tail -n +4 \
      | xargs -r rm -rf
    echo "✅ Remote web published: $REMOTE_RELEASE"
  fi
fi

# --- Flip the binary release live --------------------------------------------
if [ -n "${VK_RELEASES_DIR:-}" ] && [ "${VK_RELEASES_DEFER_FLIP:-0}" != "1" ]; then
  # Serialize the previous-repoint + current-flip + prune across concurrent
  # builders (CI + local rebuild can race): without the lock, one builder's
  # prune could snapshot current/previous, lose the race to another's flip,
  # and delete a directory that just became `current`. Same flock-a-real-file
  # pattern as the npx-cli/dist publish above.
  (
    flock --exclusive 200

    # Keep the outgoing release reachable as `previous` (one-step rollback)
    # before flipping `current`. A crash between the two flips leaves
    # previous == current — harmless.
    OLD_CURRENT="$(readlink -f "${VK_RELEASES_DIR}/current" 2>/dev/null || true)"
    if [ -n "$OLD_CURRENT" ] && [ -e "$OLD_CURRENT" ]; then
      ln -sfn "$OLD_CURRENT" "${VK_RELEASES_DIR}/.previous-${BUILD_ID}"
      mv -Tf "${VK_RELEASES_DIR}/.previous-${BUILD_ID}" \
        "${VK_RELEASES_DIR}/previous"
    fi

    # Atomic repoint, same protocol as the remote-web publish above: readers
    # always resolve either the old or the new complete release.
    ln -sfn "$RELEASE" "${VK_RELEASES_DIR}/.current-${BUILD_ID}"
    mv -Tf "${VK_RELEASES_DIR}/.current-${BUILD_ID}" "${VK_RELEASES_DIR}/current"

    # Retain the 3 newest releases, but never prune what current/previous
    # still point at — a rollback target must stay on disk.
    KEEP_CURRENT="$(readlink -f "${VK_RELEASES_DIR}/current" 2>/dev/null || true)"
    KEEP_PREVIOUS="$(readlink -f "${VK_RELEASES_DIR}/previous" 2>/dev/null || true)"
    ls -1dt "${VK_RELEASES_DIR}"/build-* 2>/dev/null \
      | tail -n +4 \
      | while IFS= read -r old; do
          old_abs="$(readlink -f "$old" 2>/dev/null || true)"
          if [ "$old_abs" != "$KEEP_CURRENT" ] && [ "$old_abs" != "$KEEP_PREVIOUS" ]; then
            rm -rf "$old"
          fi
        done
  ) 200>"${VK_RELEASES_DIR}/.publish-lock"
  echo "✅ Binary release published: $RELEASE"
elif [ -n "${VK_RELEASES_DIR:-}" ]; then
  echo "✅ Binary release staged (live flip deferred): $RELEASE"
fi

echo ""
echo "🚀 To test locally, run:"
echo "   cd npx-cli && node bin/cli.js                # browser mode (default)"
echo "   cd npx-cli && node bin/cli.js --desktop       # desktop mode (requires --desktop or --all build flag)"
