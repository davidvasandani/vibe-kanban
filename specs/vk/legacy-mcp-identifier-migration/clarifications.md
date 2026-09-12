# Clarifications

## Resolved

1. Migration is proposed on read and committed only by save; GET remains
   non-mutating.
2. The existing `suggested_server_identifier` function is authoritative.
3. The legacy key becomes `display_name` unless an explicit sidecar label already
   exists, in which case that label remains authoritative.
4. If the safe identifier already exists anywhere, automatic migration is
   refused and surfaced as a conflict.
5. The write must preflight every affected profile before changing any file.

## Open questions

None.
