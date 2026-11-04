# Technical plan

Apply `/speckit.plan` using the task pointer `specs/vk/f6e2-support-github-a` and Vibe Kanban's constitution.

## Design
Extend `crates/services/src/services/github_owner_tokens.rs` with optional App metadata on requests/views and a versioned JSON payload inside the existing encrypted value. Plain legacy values remain PATs. The App payload contains decimal IDs and a normalized op reference only. No database schema change is needed.

Add `crates/services/src/services/github_app.rs`. Use the existing jsonwebtoken 10 dependency version with Rust crypto to sign RS256 JWTs. Resolve the key on cache miss, verify GET /app and GET /app/installations/{id}, compare installation account with configured owner, POST access_tokens, and retain effective permissions. Cache per row identity and encrypted payload, with five-minute expiry margin and single-flight synchronization. Invalidation prevents stale completion from being cached. Bound HTTP timeout and redact errors.

Integrate `github_credentials.rs` so existing server git/PR call sites receive the minted token. Health keeps existing top-level status vocabulary and adds App stage evidence. Missing required permissions produces unhealthy status without hiding which stage succeeded.

For workspace commands, use an owner-scoped, expiring opaque capability issued at launch and resolved by the coordinator. Bind it to the row identity and credential revision, so update/delete revokes it. Put the capability in an environment variable whose name survives Codex filtering, never in arguments or git config. The broker accepts only this capability, returns a token with no-store, and is separate from the management API. Private keys remain on the coordinator. Workers substitute their reachable coordinator URL. gh and git call the broker at command time; failures refuse authentication. Keep PAT behavior unchanged. Capabilities expire after seven days and require a new session afterwards.

Extend `GitHubOwnerTokensCard.tsx` and its tests, generated shared types, and machine-client health access as needed. Document setup in the existing GitHub credential guide and wiki.

## Validation and constitution
Tests cover legacy payloads, validation, App identity/permissions, token expiry, concurrent requests, settings invalidation, grant revocation/expiry, secret-safe failures, real gh/git routing fixtures, and Settings creation/editing. Reuse owner routing and secret storage, preserve shared frontend boundaries and fail-closed behavior. No admin bypass option. Live tests depend on installation provisioning.
