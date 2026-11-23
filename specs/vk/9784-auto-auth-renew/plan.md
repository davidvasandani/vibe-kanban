# Technical plan

Applied `/speckit.plan`. See implementation-plan.md for dependency sequence.

## Approach
Use a registered async gateway provider in crates/services/src/services/reauth/gateway.rs;
server implementation in crates/server/src/mcp_gateway/reauth.rs owns the bound
connection inventory and existing gateway refresh/store functions. Register it
before the existing sweep starts. Extend reauth/mod.rs with identity, provider
discovery, due-selection and verified execution. Add tests beside each module.
Reuse executors/src/mcp_oauth.rs for OAuth discovery, PKCE, exchange and refresh;
reuse services/entra_mint browser profile/driver, with exact callback state checks.
Update crates/mcp/src/task_server tools/instructions and the Settings auth group.
Fix wake-up route extraction within the existing scoped route, without weakening
workspace/session authorization.

## Data model
Existing mcp_gateway_connections already contains expires_at, credential_version,
owner_user_id, machine_id and encrypted_credentials. Persist expiry on connect and
refresh. Keep gateway_token_hash unchanged on unattended repair. Existing expiry
may be absent; probe those rows and derive a scheduling hint from JWT exp when
available (never use unsigned payload claims as authentication proof).

## Contracts
Target wire id mcp-gateway:<name-or-uuid>; kind mcp_gateway. Provider discovery
returns authentication state and separate renewal-due eligibility. Run repairs
are bounded, service-owned, serialized and independently verified. Error JSON
preserves token_expired/invalid_token kind and adds reauth_target.

## Constitution check
Reuse existing registry/refusal gates and secret store. Preserve immutable owner
binding and compare-and-swap writes. No new dependencies or upstream services.

## Risks
Browser grant may require consent; refresh failures cannot be blindly retried.
An initialize probe must parse protocol success, not merely HTTP 200. Concurrent
manual reconnect/disconnect must win over stale automatic results. No claim of
live sgsc/Cloudflare recovery without a non-destructive authenticated check.
