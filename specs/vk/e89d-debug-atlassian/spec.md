# Feature specification: Reliable MCP OAuth reconnect

Status: Specified

## Summary
Reconnect a configured shared MCP server after its identifier has changed without creating an unreachable replacement OAuth connection. Atlassian Rovo exposes the failure as successful authorization followed by no matching assignments and continued HTTP 401 responses.

## User story
As a user, I can reconnect my existing server and have its assigned agents use the renewed authorization even after its display/configuration identifier was normalized.

## Functional requirements
- FR-1: Reconnect retains the existing owner-bound connection identity and local capability.
- FR-2: Initial connect continues to create a deterministic shared connection.
- FR-3: Completion updates only assignments still targeting the intended upstream or gateway; unrelated/replaced/deleted assignments are not recreated.
- FR-4: Automatic callback and pasted callback share the same identity behavior.
- FR-5: Missing or inaccessible bound connections fail without silently creating a replacement or exposing credentials.

## Acceptance criteria
- Regression tests distinguish a historical gateway ID from the ID derived after renaming and retain the historical ID.
- Initial connection identity remains deterministic.
- URL matching covers direct upstream, gateway port changes, and unrelated gateways.
- Both completion entry points propagate original connection identity.

## Out of scope
Upstream Atlassian changes, other services, live-agent reload redesign, and broad gateway status redesign. Stored connected status is distinct from a successful live probe.

## Open questions
None; preserve existing connection rather than migrate or disconnect it implicitly.

## Clarification record
Retain the owner-bound gateway UUID on reconnect; server names are mutable configuration identifiers, not evidence that a fresh OAuth connection is needed. Preserve the existing distinction between stored gateway state and live connectivity. Live browser authorization remains a post-deployment verification step.
