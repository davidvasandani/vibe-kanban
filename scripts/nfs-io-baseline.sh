#!/usr/bin/env bash
# Sample NFS and git pressure on a Vibe Kanban cluster node.
#
# Usage (on the coordinator): scripts/nfs-io-baseline.sh [seconds] [service]
#   seconds  sampling window, default 60
#   service  systemd unit whose direct git children are counted,
#            default vibe-kanban-dev.service
#
# Prints load, mean D-state task count, git spawns by the server (lower bound:
# 0.2 s polling misses very short processes), PSI, and NFS per-op rates and
# mean RTT for the shared VibeKanban mount from /proc/self/mountstats.
# Read-only. See docs/analysis/coordinator-nfs-io-pressure.md.
set -euo pipefail

DUR=${1:-60}
UNIT=${2:-vibe-kanban-dev.service}
MOUNT_RE=${NFS_MOUNT_RE:-'VibeKanban'}

snap() {
  awk -v re="$MOUNT_RE" '
    /^device / { f = ($0 ~ re) }
    f && /^[[:space:]]+[A-Z_]+: [0-9]/ { sub(":", "", $1); print $1, $2, $7, $8 }
  ' /proc/self/mountstats
}

srv=$(systemctl show -p MainPID --value "$UNIT")
a=$(mktemp) b=$(mktemp)
trap 'rm -f "$a" "$b"' EXIT

snap | sort > "$a"
declare -A seen
spawns=0 dstate=0 samples=0
end=$((SECONDS + DUR))
while [ "$SECONDS" -lt "$end" ]; do
  for p in $(pgrep -P "$srv" git || true); do
    if [ -z "${seen[$p]:-}" ]; then seen[$p]=1; spawns=$((spawns + 1)); fi
  done
  dstate=$((dstate + $(ps -eo stat= | grep -c '^D' || true)))
  samples=$((samples + 1))
  sleep 0.2
done
snap | sort > "$b"

echo "duration_s=$DUR cores=$(nproc) loadavg=$(cut -d' ' -f1-3 /proc/loadavg)"
awk -v s="$spawns" -v d="$DUR" -v ds="$dstate" -v n="$samples" \
  'BEGIN { printf "git_spawns_per_min>=%d mean_D_state=%.1f\n", s * 60 / d, ds / n }'
echo "io:  $(head -1 /proc/pressure/io 2>/dev/null || echo unavailable)"
echo "cpu: $(head -1 /proc/pressure/cpu 2>/dev/null || echo unavailable)"
join "$a" "$b" | awk -v d="$DUR" '{
  ops = $5 - $2
  if (ops > 0) printf "%-12s %9.1f ops/s  avg_rtt_ms=%.2f\n", $1, ops / d, ($7 - $4) / ops
}' | sort -k2 -nr
