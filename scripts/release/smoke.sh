#!/usr/bin/env bash
# Prove a packaged release runs standalone on a stock Linux host.
#
# Unpacks the release archives into a scratch dir and checks that:
#   - every binary is a static executable (no ELF interpreter, so it runs on
#     any x86_64 Linux, NixOS included, without patchelf);
#   - vibe-kanban starts with an empty HOME, answers /api/health, and reports
#     the release sha from /api/info.
#
# Usage: smoke.sh <assets-dir>    (reads release.json for the expected sha)
set -euo pipefail

assets="${1:?usage: smoke.sh <assets-dir>}"
sha="$(jq -r .sha "$assets/release.json")"
work="$(mktemp -d)"
server_pid=""
cleanup() {
  [ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null || true
  rm -rf "$work"
}
trap cleanup EXIT

mkdir -p "$work/bin" "$work/home"
for archive in "$assets"/*-linux-x64.tar.gz; do
  tar -xzf "$archive" -C "$work/bin"
done

for name in $(jq -r '.binaries[]' "$assets/release.json"); do
  bin="$work/bin/$name"
  [ -x "$bin" ] || { echo "smoke: $name missing from its archive" >&2; exit 1; }
  if readelf -lW "$bin" | grep -q 'Requesting program interpreter'; then
    echo "smoke: $name is dynamically linked:" >&2
    readelf -lW "$bin" | grep 'program interpreter' >&2
    exit 1
  fi
  echo "smoke: $name: $(file -b "$bin" | cut -d, -f1-2)"
done

port=38123
log="$work/server.log"
env -i \
  PATH=/usr/bin:/bin \
  HOME="$work/home" \
  XDG_DATA_HOME="$work/home/.local/share" \
  XDG_CONFIG_HOME="$work/home/.config" \
  XDG_CACHE_HOME="$work/home/.cache" \
  BACKEND_PORT="$port" \
  HOST=127.0.0.1 \
  "$work/bin/vibe-kanban" > "$log" 2>&1 &
server_pid=$!

deadline=$((SECONDS + 90))
until curl -fsS -m 2 -o /dev/null "http://127.0.0.1:$port/api/health"; do
  if ! kill -0 "$server_pid" 2>/dev/null || [ "$SECONDS" -ge "$deadline" ]; then
    echo "smoke: vibe-kanban did not become healthy; log tail:" >&2
    tail -n 50 "$log" >&2
    exit 1
  fi
  sleep 1
done

version="$(curl -fsS -m 10 "http://127.0.0.1:$port/api/info" | jq -r '.data.version // empty')"
if [ "$version" != "$sha" ]; then
  echo "smoke: /api/info version is '${version:-<none>}', expected $sha" >&2
  exit 1
fi
echo "smoke: vibe-kanban started standalone and reports $version"
