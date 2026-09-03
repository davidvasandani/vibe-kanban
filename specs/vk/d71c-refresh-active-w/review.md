# Independent Codex Review

`codex review --base origin/main` reported one significant finding:

- **P2:** the tool schema digest serialized `serde_json::Value` directly while
  the workspace enables `preserve_order`, so semantically identical schemas
  with different JSON object key order could produce different fingerprints.

The implementation now recursively sorts every JSON object key before hashing
input/output schemas. A regression test supplies equivalent nested schemas in
different key orders and proves their inventory evidence is identical.

The repeat review found a second significant issue: authentication-failed
servers could publish an empty names list and empty-inventory fingerprint even
though capability discovery was unavailable. Those evidence fields now remain
`None` for `FailedUnavailable`, matching the failure-retention contract instead
of implying deliberate tool removal.

The next repeat review found a rolling-upgrade compatibility issue: older
workers omit the new optional evidence fields, which caused deserialization to
drop their complete snapshot. Both fields now use serde defaults, and a fixture
proves an older worker payload decodes with unknown evidence instead of becoming
an empty successful server list.

Post-fix verification:

- `cargo fmt --all --check`: passed.
- `cargo test -p executors mcp_inventory_tests --lib -j1`: 4 passed.
- `cargo clippy -p executors --lib -j1 -- -D warnings`: passed.

Final repeat result: **no actionable correctness defects identified**.
