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
  --bin server --bin vibe-kanban-mcp --bin review

echo "Building Remote API binary..."
# crates/remote is excluded from the cargo workspace (exclude = [...]), so it
# has to be built via its own manifest. --target-dir keeps its output next to
# the workspace bins regardless of whether CARGO_TARGET_DIR is exported.
cargo build --release --manifest-path crates/remote/Cargo.toml \
  --target-dir "$CARGO_TARGET_DIR" --bin remote

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
  # Atomic repoint: stage a new "current" symlink, then rename it over the old
  # one. rename(2) is atomic, so the live site never resolves to a half-copied
  # tree. Both paths live in the releases dir (same filesystem, builder-owned).
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

echo ""
echo "🚀 To test locally, run:"
echo "   cd npx-cli && node bin/cli.js                # browser mode (default)"
echo "   cd npx-cli && node bin/cli.js --desktop       # desktop mode (requires --desktop or --all build flag)"
