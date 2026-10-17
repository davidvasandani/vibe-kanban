# Technical plan

Spec: ./spec.md

## Approach
In `crates/server/src/routes/mcp_auth.rs`, resolve a configured gateway UUID against the current owner/machine binding at OAuth start. Keep it in PendingFlow and ExchangeContext through both completion paths. Use it when storing refreshed credentials; derive the existing deterministic UUID only for first-time connections. Continue matching current assignments using the retained gateway path. Extract small identity/matching helpers for regression tests.

## Data model and contracts
Only process-local OAuth flow state gains an optional existing connection UUID. No database migration, public API, dependency, or generated-type changes. Public callback and manual completion contracts stay unchanged. Missing bound connections produce a secret-safe error at start.

## Research
The existing UUIDv5 input includes server_name. Explicit legacy identifier normalization preserves gateway URLs while changing configuration keys. Completion currently hashes the new name, stores a new row, then matches only the new gateway path or direct upstream. Existing assignments contain the old path, so none match; the original row still serves stale credentials. Native snapshots remain the implementation's settings authority; no new registry is required.

## Constitution check
Small existing-path fix, identity retained, tests before completion, secrets remain encrypted. No deviations. A connected database row does not prove successful upstream access.

## Risks
Live Atlassian authorization requires the user's browser. Existing cross-file persistence is not transactional; this change does not redesign it. Concurrent deletion/replacement remains excluded by matching current assignments. Configuration changed during a flow must not be recreated.
