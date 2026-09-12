#!/usr/bin/env bash
set -euo pipefail

repo_root=$(git rev-parse --show-toplevel)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/repo" "$tmp/releases/good/bin" "$tmp/bin"
git -C "$tmp/repo" init -q
git -C "$tmp/repo" config user.email test@example.com
git -C "$tmp/repo" config user.name Test
touch "$tmp/repo/source"
git -C "$tmp/repo" add source
git -C "$tmp/repo" commit -qm initial
sha=$(git -C "$tmp/repo" rev-parse HEAD)

printf '{"sha":"%s","build_id":"good"}\n' "$sha" > "$tmp/releases/good/release.json"
ln -s "$tmp/releases/good" "$tmp/releases/current"

cat > "$tmp/bin/systemctl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$TEST_STATE/systemctl.log"
EOF
cat > "$tmp/bin/curl" <<'EOF'
#!/usr/bin/env bash
count_file="$TEST_STATE/curl-count"
count=$(cat "$count_file" 2>/dev/null || printf 0)
count=$((count + 1))
printf '%s' "$count" > "$count_file"
# The candidate remains unhealthy until the rollout restores the known-good
# symlink; this exercises the timeout and rollback paths deterministically.
[[ "$(readlink -f "$VK_RELEASES_DIR/current")" != "$VK_RELEASES_DIR/candidate" ]]
EOF
chmod +x "$tmp/bin/systemctl" "$tmp/bin/curl"

build_command=$(cat <<'EOF'
candidate="$VK_RELEASES_DIR/candidate"
mkdir -p "$candidate/bin"
for artifact in vibe-kanban vibe-kanban-mcp vibe-kanban-review remote relay-server vibe-kanban-worker; do
  printf '#!/bin/sh\n' > "$candidate/bin/$artifact"
  chmod +x "$candidate/bin/$artifact"
done
printf '{"sha":"%s","build_id":"candidate"}\n' "$(git rev-parse HEAD)" > "$candidate/release.json"
EOF
)

set +e
TEST_STATE="$tmp" \
VK_ROLLOUT_REPO="$tmp/repo" \
VK_ROLLOUT_BUILD_TREE="$tmp/build-tree" \
VK_ROLLOUT_TARGET_DIR="$tmp/cluster-target" \
VK_RELEASES_DIR="$tmp/releases" \
VK_ROLLOUT_SYSTEMCTL="$tmp/bin/systemctl" \
VK_ROLLOUT_CURL="$tmp/bin/curl" \
VK_ROLLOUT_UNITS=test.service \
VK_ROLLOUT_HEALTH_CHECKS='test.service|http://test/health' \
VK_ROLLOUT_HEALTH_TIMEOUT_SECONDS=1 \
VK_ROLLOUT_HEALTH_RETRY_SECONDS=0 \
VK_ROLLOUT_BUILD_COMMAND="$build_command" \
  "$repo_root/scripts/cluster-rollout.sh" "$sha" > "$tmp/output" 2>&1
status=$?
set -e

[[ "$status" -ne 0 ]]
[[ "$(readlink -f "$tmp/releases/current")" == "$tmp/releases/good" ]]
[[ "$(wc -l < "$tmp/systemctl.log")" -eq 2 ]]
grep -q 'candidate failed health checks and was rolled back' "$tmp/output"
printf 'cluster rollout rollback test: PASS\n'

# Reset the fake health probe to always succeed and prove the candidate remains
# live after a successful gate.
cat > "$tmp/bin/curl" <<'EOF'
#!/usr/bin/env bash
exit 0
EOF
rm -rf "$tmp/releases/candidate" "$tmp/build-tree"
rm -f "$tmp/releases/current" "$tmp/systemctl.log"
ln -s "$tmp/releases/good" "$tmp/releases/current"

TEST_STATE="$tmp" \
VK_ROLLOUT_REPO="$tmp/repo" \
VK_ROLLOUT_BUILD_TREE="$tmp/build-tree" \
VK_ROLLOUT_TARGET_DIR="$tmp/cluster-target" \
VK_RELEASES_DIR="$tmp/releases" \
VK_ROLLOUT_SYSTEMCTL="$tmp/bin/systemctl" \
VK_ROLLOUT_CURL="$tmp/bin/curl" \
VK_ROLLOUT_UNITS=test.service \
VK_ROLLOUT_HEALTH_CHECKS='test.service|http://test/health' \
VK_ROLLOUT_BUILD_COMMAND="$build_command" \
  "$repo_root/scripts/cluster-rollout.sh" "$sha" > "$tmp/success-output" 2>&1

[[ "$(readlink -f "$tmp/releases/current")" == "$tmp/releases/candidate" ]]
[[ "$(wc -l < "$tmp/systemctl.log")" -eq 1 ]]
grep -q 'deploy complete and healthy' "$tmp/success-output"
printf 'cluster rollout success test: PASS\n'
