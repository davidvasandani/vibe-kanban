# Technical specification: 9784-auto-auth-renew

## Problem and scope
Vibe Kanban proxies MCP through durable per-entry bearer capabilities, but upstream OAuth credentials can expire without an auto-auth target. Repairing sgsc backend grants cannot renew the gateway's own upstream grant. Change only the Vibe Kanban service, preserving owner/machine binding and encrypted credential storage. Investigate the reported cancel_wakeup route collision as a separate regression within this service.

## Required behavior
Expose each connected gateway OAuth entry as a server-defined mcp-gateway target (friendly server name when unambiguous, UUID otherwise), kind mcp_gateway, swept true. Derive authentication from expiry metadata and bounded initialize probes, distinguishing explicit authentication rejection from network failure. Keep existing gateway bearer capabilities stable while replacing encrypted upstream credentials.

Renew using stored refresh tokens first; reuse the persistent vk-entra browser and existing OAuth discovery/PKCE exchange for silent sign-in when refresh is unavailable or definitively rejected. Only explicit human-required conditions latch refusal. Bound network/browser work, redact secrets, serialize concurrent renewal, and reject stale writes or resurrection after disconnect. Verify initialize before reporting success.

The existing sweep must select credentials due before the next interval with safety margin; proxy requests also repair rejected credentials safely. Persist expiry from token responses, retaining refresh tokens when providers omit replacements. Gateway authentication error bodies name the reauth target without leaking credentials; instructions distinguish gateway OAuth from sgsc backend onboarding and explain tool reload/restart behavior.

## Validation
Cover target parsing/discovery and owner isolation; expiry horizon and unknown transport state; refresh rotation and omitted refresh token; concurrent repair/disconnect; stable bearer after renewal; probe-based success; browser MFA/consent refusal; actionable token_expired/invalid_token; wake-up cancellation with workspace/session context. Run focused Rust tests and repository-required setup/format/checks. Independently review the diff before PR and merge. Do not expire production credentials to manufacture a test outage; use isolated mock upstream fixtures for forced expiry, and perform non-destructive live checks where access is available.
