# Debug Atlassian Rovo MCP reconnect

Task: vk/e89d-debug-atlassian

## Observed problem
The supplied screenshot shows Atlassian Rovo marked shared-gateway connected while executor connection tests return HTTP 401 invalid_token. Completing OAuth reports “No matching MCP assignments remained after OAuth completed”.

## Requirements
Trace the OAuth start, exchange, shared gateway persistence, settings assignments, and connection-test paths. Correct the authoritative assignment update so reconnect applies credentials to the configured server and assigned executors. Preserve unrelated definitions and assignments, detect concurrent deletion or replacement, and never expose credentials in errors or diagnostics. Connected status must reflect its documented meaning without concealing a failed reconnect.

## Scope
Vibe Kanban only. Hosting changes, if evidenced necessary, are limited to homelab/modules/vibe-kanban-rebuild.nix. No changes to Atlassian or other services.

## Acceptance
Add focused regression coverage for initial shared connection, reconnect, missing/replaced assignments, and unaffected unrelated servers as applicable to the diagnosed cause. Run required formatting and relevant checks, independent Codex review, record reusable knowledge, then open and merge a PR against the base branch. A live OAuth grant may require the user's browser; automated verification must not claim to prove a live grant.

## Diagnosis
Native executor snapshots remain the implementation’s settings authority. Reconnect re-derived the gateway UUID from a renamed server identifier, storing credentials in a different row and failing to match original gateway assignments. Preserve the owner-bound UUID resolved at OAuth start.
