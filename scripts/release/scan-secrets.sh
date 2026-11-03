#!/usr/bin/env bash
# Fail if a token-shaped string appears in a release artifact.
#
# The repository and its GitHub Releases are public, and the release build
# compiles some environment values into the binaries (option_env!). This scan
# is the last gate before upload: it unpacks every archive and searches the
# raw bytes of every file for credential shapes. A match fails the job.
#
# Matches are reported by file and pattern name with the value redacted to its
# first four characters, so the scan never republishes what it found in the
# (public) job log.
#
# Usage:
#   scan-secrets.sh <file-or-dir>...   scan artifacts (archives are unpacked)
#   scan-secrets.sh --self-test        prove each pattern still fires
set -euo pipefail

# name|PCRE. Keep each pattern anchored on a vendor prefix so arbitrary binary
# data does not match by accident.
PATTERNS=(
  'github-token|gh[pousr]_[A-Za-z0-9]{36,}'
  'github-fine-grained-pat|github_pat_[A-Za-z0-9_]{60,}'
  'aws-access-key-id|(?<![A-Z0-9])(AKIA|ASIA)[0-9A-Z]{16}(?![A-Z0-9])'
  'slack-token|xox[abposr]-[0-9]{6,}-[0-9A-Za-z-]{6,}'
  'slack-webhook|hooks\.slack\.com/services/T[0-9A-Z]{6,}/B[0-9A-Z]{6,}/[0-9A-Za-z]{12,}'
  'anthropic-key|sk-ant-[A-Za-z0-9_-]{20,}'
  'openai-key|sk-(proj-)?[A-Za-z0-9_-]{20,}T3BlbkFJ[A-Za-z0-9_-]{20,}'
  'onepassword-service-account|ops_eyJ[A-Za-z0-9_-]{20,}'
  'private-key|-----BEGIN ((RSA|EC|DSA|OPENSSH|ENCRYPTED) )?PRIVATE KEY-----\r?\n[A-Za-z0-9+/=]{40,}'
  'sentry-dsn|https://[0-9a-f]{32}@[A-Za-z0-9.-]*sentry\.io/[0-9]+'
  'posthog-project-key|phc_[A-Za-z0-9]{32,}'
  'google-api-key|AIza[0-9A-Za-z_-]{35}'
  'stripe-live-key|[sr]k_live_[0-9A-Za-z]{24,}'
  'npm-token|npm_[A-Za-z0-9]{36}'
)

# Total matches (and scan errors) across every file (a return code would wrap
# at 256).
FOUND=0

# Returns 1 when the file holds at least one match.
scan_file() {
  local file="$1" label="$2" entry name re hits=0
  for entry in "${PATTERNS[@]}"; do
    name="${entry%%|*}"
    re="${entry#*|}"
    # -z: NUL-delimited records, so a PEM body's newline stays inside one
    # record and multi-line shapes can match. grep exits 1 for "no match";
    # anything above that means the file was NOT scanned, which must block
    # publication rather than read as clean.
    local out rc=0
    out="$(mktemp)"
    LC_ALL=C grep -aozP -- "$re" "$file" > "$out" 2> "$out.err" || rc=$?
    if [ "$rc" -gt 1 ]; then
      printf 'SCAN ERROR: %s: %s: grep exit %s: %s\n' "$label" "$name" "$rc" "$(head -c 200 "$out.err")" >&2
      hits=$((hits + 1))
    fi
    while IFS= read -r -d '' match; do
      [ -n "$match" ] || continue
      printf 'SECRET-SHAPED STRING: %s: %s (%s…)\n' "$label" "$name" "${match:0:4}" >&2
      hits=$((hits + 1))
    done < "$out"
    rm -f "$out" "$out.err"
  done
  FOUND=$((FOUND + hits))
  [ "$hits" -eq 0 ]
}

# Archives are unpacked and their members scanned (compressed bytes would hide
# any token), recursively, wherever they appear — top level or inside a dir.
scan_archive() {
  local archive="$1" label="$2" work clean=0
  work="$(mktemp -d)"
  if ! tar -xzf "$archive" -C "$work"; then
    echo "scan-secrets: cannot unpack $label" >&2
    rm -rf "$work"
    FOUND=$((FOUND + 1))
    return 1
  fi
  scan_tree "$work" "$label!" || clean=1
  rm -rf "$work"
  return "$clean"
}

scan_tree() {
  local root="$1" prefix="$2" clean=0 file
  while IFS= read -r -d '' file; do
    case "$file" in
      *.tar.gz | *.tgz) scan_archive "$file" "$prefix${file#"$root"/}" || clean=1 ;;
      *) scan_file "$file" "$prefix${file#"$root"/}" || clean=1 ;;
    esac
  done < <(find "$root" -type f -print0)
  return "$clean"
}

scan_path() {
  local path="$1"
  if [ -d "$path" ]; then
    scan_tree "$path" ""
  else
    case "$path" in
      *.tar.gz | *.tgz) scan_archive "$path" "$(basename "$path")" ;;
      *) scan_file "$path" "$(basename "$path")" ;;
    esac
  fi
}

self_test() {
  local dir rc=0 entry name sample
  dir="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '$dir'" RETURN
  # One synthetic sample per pattern, assembled at runtime so this script
  # itself never contains a literal token shape.
  declare -A samples=(
    [github-token]="gh""p_$(printf 'a%.0s' {1..36})"
    [github-fine-grained-pat]="github""_pat_$(printf 'B%.0s' {1..70})"
    [aws-access-key-id]="AK""IA$(printf 'Z%.0s' {1..16})"
    [slack-token]="xo""xb-1234567890-abcdefghijkl"
    [slack-webhook]="hooks.slack.com/services/""T0123456/B0123456/abcdefghijklmnop"
    [anthropic-key]="sk-""ant-$(printf 'c%.0s' {1..30})"
    [openai-key]="sk-""proj-$(printf 'd%.0s' {1..24})T3Blbk""FJ$(printf 'e%.0s' {1..24})"
    [onepassword-service-account]="ops""_eyJ$(printf 'f%.0s' {1..30})"
    [private-key]="$(printf -- '-----BEGIN RSA PRIVATE''%s\n%s' ' KEY-----' "$(printf 'M%.0s' {1..64})")"
    [sentry-dsn]="https://$(printf '0%.0s' {1..32})@o1.ingest.sentry"".io/123"
    [posthog-project-key]="ph""c_$(printf 'g%.0s' {1..40})"
    [google-api-key]="AI""za$(printf 'h%.0s' {1..35})"
    [stripe-live-key]="sk_""live_$(printf 'i%.0s' {1..30})"
    [npm-token]="np""m_$(printf 'j%.0s' {1..36})"
  )
  for entry in "${PATTERNS[@]}"; do
    name="${entry%%|*}"
    sample="${samples[$name]:-}"
    if [ -z "$sample" ]; then
      echo "self-test: no sample for pattern $name" >&2
      rc=1
      continue
    fi
    printf '\x00\x01binary-prefix%s\x00suffix' "$sample" > "$dir/$name.bin"
    if scan_file "$dir/$name.bin" "$name.bin" 2>/dev/null; then
      echo "self-test: pattern $name did NOT fire on its sample" >&2
      rc=1
    fi
  done
  # And the archive path: a token inside a tarball must be found too.
  mkdir -p "$dir/pkg"
  printf '%s' "${samples[github-token]}" > "$dir/pkg/bin"
  tar -czf "$dir/pkg.tar.gz" -C "$dir/pkg" bin
  if scan_path "$dir/pkg.tar.gz" 2>/dev/null; then
    echo "self-test: token inside an archive was NOT found" >&2
    rc=1
  fi
  # The workflow passes the assets DIRECTORY; archives inside it must be
  # unpacked too, not scanned as compressed bytes.
  mkdir -p "$dir/assets"
  cp "$dir/pkg.tar.gz" "$dir/assets/"
  if scan_path "$dir/assets" 2>/dev/null; then
    echo "self-test: token inside an archive inside a directory was NOT found" >&2
    rc=1
  fi
  # A file grep cannot read must fail the scan, not pass it.
  if scan_file "$dir/does-not-exist.bin" missing.bin 2>/dev/null; then
    echo "self-test: an unscannable file was reported clean" >&2
    rc=1
  fi
  # And a clean file must pass.
  printf 'nothing to see here' > "$dir/clean.bin"
  if ! scan_file "$dir/clean.bin" clean.bin; then
    echo "self-test: clean file reported a match" >&2
    rc=1
  fi
  [ "$rc" -eq 0 ] && echo "scan-secrets self-test: all ${#PATTERNS[@]} patterns fire; clean input passes"
  return "$rc"
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit
fi

if [ "$#" -eq 0 ]; then
  echo "usage: $0 <file-or-dir>... | --self-test" >&2
  exit 2
fi

for path in "$@"; do
  scan_path "$path" || true
done
if [ "$FOUND" -ne 0 ]; then
  echo "scan-secrets: $FOUND secret-shaped string(s) found; refusing to publish" >&2
  exit 1
fi
echo "scan-secrets: no secret-shaped strings in $# path(s)"
