# Feature: unattended MCP gateway credential renewal

Applied `/speckit.specify` for this task (correcting the template's stale feature directory).

## User stories
- An unattended agent discovers and repairs a gateway authentication failure using the target named in the error.
- A running session continues using its gateway bearer after upstream renewal.
- An operator sees truthful authentication state and human-required refusals.

## Requirements
- FR1: List connected OAuth entries as mcp_gateway targets, supporting sgsc-mcp by name and UUID identity, with swept=true and evidence-backed auth_state.
- FR2: Renew via refresh grants, otherwise silent browser SSO; preserve scope, ownership, encrypted storage and stable bearer.
- FR3: Select upcoming expiry before the next sweep and independently verify authentication after repair.
- FR4: Treat transport errors as unknown and reserve refused for explicit human intervention.
- FR5: Annotate token_expired and invalid_token errors with reauth_target and document recovery/restart.
- FR6: Cancel a wake-up using its required workspace/session context without duplicate route-field errors.

## Acceptance
- Isolated forced expiry is repaired and initialize succeeds using the same gateway bearer.
- Valid near-expiry credentials are proactively renewed; omitted refresh replacement preserves the previous grant.
- Failed probes, concurrent rotation/disconnect and human-required flows do not produce false success.
- Cloudflare follows the same OAuth path; absent renewable grant or interactive consent is explained accurately.
- Scoped wake-up cancellation reaches the intended row and rejects a wrong scope.

## Out of scope
Changes to upstream MCP services, new MFA bypass, and production credential expiry experiments.

## Open questions
None; reuse existing trusted browser automation and OAuth validation boundaries.

## Clarification record
Applied `/speckit.clarify`: choose stable gateway bearer replacement, never
rewrite agent configuration. Names are aliases only when unambiguous within
owner/machine scope; UUIDs remain available. Consent or interactive MFA is a
human-required refusal, whereas timeouts, missing configuration and rejected
refresh grants alone are not. Regression tests force expiry in isolated data.
