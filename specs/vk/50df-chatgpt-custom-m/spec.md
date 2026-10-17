# Feature Specification: ChatGPT custom-connector access to the Vibe Kanban MCP

**Feature dir**: `specs/vk/50df-chatgpt-custom-m/`
**Status**: Draft

## Summary

The owner tried to add Vibe Kanban to ChatGPT as a custom MCP connector
(`https://vibe.vasandani.dev/mcp`, Authentication "No Auth"). ChatGPT
reported "Error creating connector". The Vibe Kanban MCP is reachable today
only by clients that present Cloudflare Access service-token headers and a
static origin bearer. ChatGPT can authenticate a connector only with OAuth
(or no auth), so it can never reach the MCP. This feature lets the owner
connect ChatGPT by signing in once through a browser consent step. After
that, ChatGPT can use every Vibe Kanban MCP tool. Existing MCP clients and
the human UI keep working exactly as before, and no part of Vibe Kanban
becomes reachable without authentication.

## User Stories

- As the Vibe Kanban owner, I want to add Vibe Kanban as a ChatGPT connector
  with OAuth so that I can manage issues, workspaces and sessions from
  ChatGPT (including on my phone).
- As the owner, I want the ChatGPT sign-in to require my existing Vibe Kanban
  SSO login and an explicit approval, so that nobody else can link their
  ChatGPT to my Vibe Kanban.
- As the owner, I want to see and revoke ChatGPT's (or any OAuth client's)
  access so that a lost device or retired connector can be cut off.
- As an existing MCP consumer (Claude Code, Codex, Hermes, Ohana, VK agents),
  I want my current `/mcp` access to keep working unchanged.

## Functional Requirements

- FR-1: A public MCP resource URL dedicated to OAuth clients exists. It
  serves the same full tool set as the existing MCP.
- FR-2: Unauthenticated requests to that resource receive a standards-based
  challenge pointing to discovery metadata, so an MCP OAuth client can find
  the authorization server without manual configuration.
- FR-3: Clients can register themselves (dynamic client registration).
  Registration validates redirect URIs and bounds its input. Abandoned
  registrations are cleaned up and the total number is capped.
- FR-4: Authorization uses the authorization-code flow with PKCE (S256
  only). The consent page is reachable only after the owner's existing
  Cloudflare Access SSO. It names the requesting client and requires an
  explicit Approve or Deny.
- FR-5: A forged or replayed consent submission, an unregistered redirect
  target, or a mismatched resource is rejected. An invalid redirect target
  is never redirected to.
- FR-6: Access tokens are short-lived. Refresh tokens rotate on use. Reuse
  of a rotated refresh token, or replay of an authorization code, revokes
  the whole grant.
- FR-7: The MCP resource accepts only valid, unexpired, unrevoked tokens
  that this Vibe Kanban issued for that resource. It accepts no other
  credential or header.
- FR-8: The owner can list grants (client name, created, last used) and
  revoke one through the authenticated Vibe Kanban API.
- FR-9: Secrets (tokens, codes, client secrets) are stored only as hashes and
  never appear in logs or error messages.
- FR-10: The feature is off unless the deployment configures a public issuer
  URL. With it off, the new endpoints do not exist.
- FR-11: At the edge, only the paths ChatGPT must reach without a browser
  session (discovery, registration, token, MCP resource) skip Access SSO,
  and only from OpenAI's published connector egress ranges. The consent
  page, the UI, and the existing `/mcp` keep their current protection.
- FR-12: The OpenAI range list is version-controlled, with a documented way
  to refresh it. The repository's edge-exposure check proves the invariants
  in FR-11.

## Out of Scope

- Changing authentication of the existing `/mcp` route or its clients.
- A graphical Settings page for grants (API only for now).
- Per-tool scopes or read-only modes (the owner chose full access).
- A public token-revocation endpoint for clients.
- Supporting OAuth clients other than through the same generic standards
  (no ChatGPT-specific code paths).

## Acceptance Criteria

- [ ] With the feature unconfigured, every new endpoint returns 404.
- [ ] Discovery documents list the issuer, the registration, authorize and
      token endpoints, S256 only, and the `mcp` scope. The resource metadata
      names the dedicated MCP URL.
- [ ] Registration rejects missing, non-https (non-loopback) or
      fragment-bearing redirect URIs and unsupported grant or response types.
      It accepts ChatGPT's shape.
- [ ] An authorize request with an unknown client or unregistered
      redirect URI renders an error page and does not redirect. Other
      protocol errors redirect with `error` and `state`.
- [ ] Approving with the wrong or a reused consent token fails. Approving
      correctly redirects with `code`, `state` and `iss`. Deny redirects with
      `access_denied`.
- [ ] A code exchange with a wrong PKCE verifier, wrong redirect URI, wrong
      client, or an expired or used code fails with `invalid_grant`. A
      replayed code revokes the tokens it minted.
- [ ] Refresh returns a new pair and invalidates the old refresh token.
      Reusing the old one revokes the grant, and its access token stops
      verifying.
- [ ] Token verification accepts a fresh access token and rejects refresh
      tokens, expired or revoked tokens, and garbage. Rejections carry the
      `WWW-Authenticate` resource-metadata challenge and a JSON-RPC error
      body.
- [ ] The edge-exposure check passes and fails if `/mcp` or the consent
      path is bypassed, or if the OAuth paths lose their IP list.
- [ ] After deployment, the owner creates the ChatGPT connector with OAuth
      and it lists Vibe Kanban tools. `/mcp` clients still connect.

## Clarifications

Resolved during `/speckit.clarify` from the investigation and the owner's
answers (scope: both repos; exposure: OpenAI ranges plus OAuth; tools: all):

- **Separate resource path, not `/mcp`.** The resource is `/oauth/mcp`. The
  existing `/mcp` origin trusts the mere presence of a Cloudflare Access
  assertion header. That is safe only while every `/mcp` request passes
  Access. Bypassing `/mcp` for OpenAI ranges would let a request from those
  ranges forge the header. A path-scoped bypass would also replace the
  hostname policy for `/mcp` and block service-token clients from other
  networks. A separate path avoids both.
- **Lifetimes.** Authorization codes last 60 s and pending consent 10 min.
  Access tokens last 1 h. Refresh tokens last 30 days, sliding through
  rotation, with no absolute cap: the owner revokes a grant explicitly
  (FR-8), and ChatGPT must not silently lose a connector it uses daily.
- **Service tokens on the bypassed paths: no.** Those paths serve only OAuth
  clients. Existing service-token clients keep using `/mcp` under the
  hostname policy. Bypassed paths admit only OpenAI's connector ranges.
  Other sources are blocked at the edge.

## Open Questions

None remain. Rollout risk to verify after deploy: ChatGPT's egress must come
from the published `chatgpt-connectors.json` ranges. If connector creation
is blocked at the edge, compare the request's source with the vendored list
and refresh it.
