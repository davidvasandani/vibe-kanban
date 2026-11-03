#!/usr/bin/env bash
# Package a linux-x64 release build into GitHub Release assets.
#
# Inputs (environment):
#   BIN_DIR          cargo output dir holding the built binaries
#   REMOTE_WEB_DIST  built packages/remote-web/dist
#   OUT_DIR          where to write the assets (created, must not exist)
#   RELEASE_SHA      full 40-hex commit sha the binaries were built from
#   RELEASE_TAG      release tag, build-<12-hex>
#   BUILD_ID         unique build identifier (gh-<run_id>-<attempt>)
#   BUILT_AT         RFC 3339 UTC build timestamp (also VK_BUILD_TIMESTAMP)
#   TARGET           rust target triple the binaries were built for
#   RUN_URL          URL of the workflow run that built this release
#
# Output, one immutable set per commit:
#   <name>-linux-x64.tar.gz   one per binary, holding just ./<name>
#   remote-web-dist.tar.gz    the remote-web frontend bundle
#   release.json              {sha, build_id, built_at, ...} — same keys the
#                             homelab release dirs use, plus provenance
#   SHA256SUMS                sha256 of every asset above
#
# Archives are deterministic (sorted, fixed owner, mtime = commit time,
# gzip -n), so rebuilding a commit with the same toolchain gives the same
# bytes for the same binaries.
set -euo pipefail

: "${BIN_DIR:?}" "${REMOTE_WEB_DIST:?}" "${OUT_DIR:?}" "${RELEASE_SHA:?}"
: "${RELEASE_TAG:?}" "${BUILD_ID:?}" "${BUILT_AT:?}" "${TARGET:?}" "${RUN_URL:?}"

case "$RELEASE_SHA" in
  *[!0-9a-f]* | "") echo "package: RELEASE_SHA must be lowercase hex" >&2; exit 1 ;;
esac
[ "${#RELEASE_SHA}" -eq 40 ] || { echo "package: RELEASE_SHA must be 40 hex chars" >&2; exit 1; }

# cargo bin name -> deployed name. The deployed names match local-build.sh's
# release layout (build-<id>/bin/*), which is what homelab services exec.
BINARIES=(
  "server:vibe-kanban"
  "vibe-kanban-mcp:vibe-kanban-mcp"
  "review:vibe-kanban-review"
  "vibe-kanban-worker:vibe-kanban-worker"
  "remote:remote"
  "relay-server:relay-server"
)

mtime="$(git log -1 --format=%ct "$RELEASE_SHA")"
mkdir "$OUT_DIR"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

det_tar() {
  # $1 = archive path, $2 = directory to archive, rest = members
  local archive="$1" dir="$2"
  shift 2
  tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$mtime" \
    --format=gnu -C "$dir" -cf - "$@" | gzip -n -9 > "$archive"
}

for pair in "${BINARIES[@]}"; do
  src="${pair%%:*}"
  name="${pair#*:}"
  if [ ! -f "$BIN_DIR/$src" ]; then
    echo "package: missing built binary $BIN_DIR/$src" >&2
    exit 1
  fi
  mkdir -p "$stage/$name"
  install -m 0755 "$BIN_DIR/$src" "$stage/$name/$name"
  det_tar "$OUT_DIR/$name-linux-x64.tar.gz" "$stage/$name" "$name"
done

if [ ! -f "$REMOTE_WEB_DIST/index.html" ]; then
  echo "package: remote-web bundle missing index.html at $REMOTE_WEB_DIST" >&2
  exit 1
fi
det_tar "$OUT_DIR/remote-web-dist.tar.gz" "$REMOTE_WEB_DIST" .

jq -n \
  --arg sha "$RELEASE_SHA" \
  --arg build_id "$BUILD_ID" \
  --arg built_at "$BUILT_AT" \
  --arg tag "$RELEASE_TAG" \
  --arg target "$TARGET" \
  --arg run_url "$RUN_URL" \
  --argjson binaries "$(printf '%s\n' "${BINARIES[@]}" | cut -d: -f2 | jq -R . | jq -s .)" \
  '{sha: $sha, build_id: $build_id, built_at: $built_at, tag: $tag,
    target: $target, platform: "linux-x64", binaries: $binaries,
    run_url: $run_url}' > "$OUT_DIR/release.json"

(cd "$OUT_DIR" && sha256sum -- *.tar.gz release.json > SHA256SUMS)
echo "package: wrote $(wc -l < "$OUT_DIR/SHA256SUMS") assets + SHA256SUMS to $OUT_DIR"
cat "$OUT_DIR/SHA256SUMS"
