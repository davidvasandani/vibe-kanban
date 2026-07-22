# Feature Specification: Debug VK MCP Connection Failure

**Feature dir**: `specs/vk/044f-debug-vk-mcp-err/`
**Status**: Implemented
**Task**: `vk/044f-debug-vk-mcp-err`

## Summary

Vibe Kanban's MCP connection test should distinguish authentication
intermediary responses, redirects, malformed protocol responses, and ordinary
transport failures instead of collapsing them into an opaque JSON parse error.
When the MCP settings screen tests a Streamable HTTP or legacy SSE server, the
probe must preserve enough safe HTTP context to explain what failed and why,
while continuing to support the existing successful JSON, SSE, and OAuth
connection flows. HTTP/SSE probes must observe redirect responses directly
rather than following them to a browser-oriented login page.

The immediate failure to address is the `Vibe Kanban` HTTP MCP assignment at
`https://vibe.vasandani.dev/mcp`, where Claude Code, Codex, and Gemini all show:

```text
invalid JSON response: expected value at line 1 column 1
```

Credential-free evidence indicates the endpoint returns a Cloudflare Access
authentication redirect with an HTML body and a `WWW-Authenticate:
Cloudflare-Access ...` challenge. The connection test should surface that as an
authentication-required condition or as an actionable redirect/non-MCP response,
not as a protocol JSON parsing failure.

## Why

The current diagnostic loses the upstream HTTP status, redirect target,
authentication challenge, content type, and safe response preview. Operators
therefore cannot tell whether:

- the configured MCP server returned malformed JSON-RPC data;
- Cloudflare Access or another intermediary returned an HTML login page;
- the endpoint is routed to the wrong service;
- credentials are missing at the edge, the origin, or both; or
- the probe followed a redirect that should have remained visible.

The failure is especially confusing because the same shared MCP assignment is
tested through multiple executors, so the UI repeats the same opaque message for
Claude Code, Codex, and Gemini. Better backend classification fixes the settings
result display, copied diagnostics, and debug-issue seeding surfaces without
requiring a new frontend contract.

## User Stories

- As a user testing an HTTP MCP server, I want authentication redirects to be
  identified as authentication problems, so I know to connect or provide the
  required access credentials instead of debugging JSON syntax.
- As an operator of an Access-protected MCP endpoint, I want the probe to
  preserve the `WWW-Authenticate` challenge, so the existing OAuth/Connect flow
  can use the server-provided authentication hint.
- As a user seeing a failed MCP test, I want a bounded and safe diagnostic that
  includes status and content type, so I can distinguish an HTML login page from
  an MCP protocol response.
- As a user configuring shared MCP assignments, I want Claude Code, Codex, and
  Gemini failures to explain the same root cause consistently, so repeated
  executor results are useful rather than noisy.
- As a security-conscious operator, I want diagnostics to avoid credentials,
  cookies, and arbitrary response headers, so test output can be copied into
  issues without leaking secrets.

## Functional Requirements

- FR-1: Streamable HTTP and legacy SSE MCP probes MUST disable automatic redirect
  following for every request made by the connection test, including Streamable
  HTTP POSTs, the legacy SSE GET, and legacy SSE message POSTs.
- FR-2: An HTTP redirect response that includes a usable `WWW-Authenticate`
  challenge MUST be classified as `auth_required`.
- FR-3: When a redirect is classified as `auth_required`, the probe MUST retain
  the challenge value in the existing `www_authenticate` field used by the
  connection flow.
- FR-4: A redirect response without a usable authentication challenge MUST remain
  a failed test result, not an OAuth-capable or successful result.
- FR-5: Failed redirect diagnostics MUST include the HTTP redirect status and a
  safe indication of the redirect destination when available. The safe redirect
  destination MUST omit query strings, fragments, userinfo, and any unparseable
  value.
- FR-6: A successful 2xx HTTP response that is neither valid MCP JSON nor MCP
  SSE MUST remain a failed test result.
- FR-7: Non-MCP 2xx diagnostics MUST include safe protocol context: HTTP status,
  response content type when present, and a bounded, sanitized body preview.
  The preview MUST be capped at 200 displayed characters.
- FR-8: The probe MUST NOT report a non-MCP HTML or login response only as a
  serde/JSON parse error.
- FR-9: Existing 401 and 403 authentication-required classification MUST keep
  working, including preservation of `WWW-Authenticate` when present.
- FR-10: Successful Streamable HTTP JSON responses MUST keep their existing
  success behavior.
- FR-11: Successful Streamable HTTP SSE responses MUST keep their existing
  success behavior.
- FR-12: Legacy SSE server probes MUST keep their existing success behavior while
  gaining the same redirect visibility and safe failure diagnostics.
- FR-13: Stdio MCP server tests, timeouts, connection failures, and unsupported
  executor results MUST not regress.
- FR-14: Test diagnostics MUST never echo configured request headers,
  Authorization values, Cloudflare Access service-token values, cookies, or
  other secret material. Diagnostics MUST NOT include arbitrary response headers;
  only HTTP status, content type, sanitized redirect destination, sanitized body
  preview, and the existing `www_authenticate` field are in scope.
- FR-15: Focused tests MUST reproduce the redirect-to-HTML failure path without
  requiring external network access.
- FR-16: Focused tests MUST cover challenged redirects, unchallenged redirects,
  non-MCP 2xx HTML responses, existing 401/403 authentication behavior, and
  successful JSON/SSE MCP responses.
- FR-17: The deployed endpoint and repository deployment configuration SHOULD be
  inspected read-only to distinguish probe behavior from a separate deployment
  or credential provisioning defect.

## Non-functional Requirements

- NF-1: Diagnostics must be actionable but bounded; body previews must be short
  enough for UI display, clipboard use, logs, and issue descriptions.
- NF-2: Sanitization must prefer omission over risky detail when response data
  could contain credentials or session material.
- NF-3: The behavior must fit the existing backend-to-UI result contract; new
  frontend result types are out of scope unless unavoidable.
- NF-4: The probe must retain the current security boundary between Cloudflare
  Access service credentials and origin bearer credentials.
- NF-5: Tests must use synthetic local HTTP fixtures and synthetic credentials
  only.
- NF-6: The implementation should remain a small extension of the existing
  hand-rolled probe and should not add top-level dependencies unless planning
  later records a constitution-compliant reason.

## Out of Scope

- Weakening, bypassing, or reconfiguring Cloudflare Access.
- Reading, committing, rotating, exposing, or migrating Cloudflare Access service
  tokens or MCP bearer tokens.
- Moving Cloudflare Access credentials into native agent configs.
- Replacing the MCP probe transport implementation wholesale.
- Changing MCP server configuration schemas or shared assignment storage.
- Treating arbitrary redirects as OAuth-capable when no authentication challenge
  is present.
- Applying production infrastructure changes unless read-only investigation
  identifies a distinct repository defect.
- Adding frontend-only special handling for this specific error string.

## Acceptance Criteria

- [ ] The screenshot scenario no longer reports only `invalid JSON response:
      expected value at line 1 column 1` for a Cloudflare Access login redirect
      or HTML login response.
- [ ] A redirect carrying a `WWW-Authenticate` challenge is returned as
      `auth_required` and exposes the challenge through the existing
      `www_authenticate` field.
- [ ] A redirect without a usable authentication challenge fails with a
      diagnostic that includes the redirect status and safe redirect context
      without query strings, fragments, userinfo, or arbitrary headers.
- [ ] A 2xx HTML or other non-MCP response fails with a diagnostic that includes
      status, content type when present, and a bounded sanitized preview of no
      more than 200 displayed characters.
- [ ] Valid MCP JSON handshake responses continue to pass.
- [ ] Valid MCP SSE handshake responses continue to pass.
- [ ] Existing 401/403 authentication-required results continue to pass and keep
      their existing Connect/OAuth behavior.
- [ ] Stdio, timeout, connection-refused, and unsupported-executor probe
      behavior remains unchanged except for any harmless diagnostic improvements.
- [ ] Focused tests cover challenged redirect, unchallenged redirect,
      redirect-to-HTML/non-MCP response, 401/403 auth-required behavior, valid
      JSON, and valid SSE without external network dependency.
- [ ] Repository formatting and relevant backend checks pass.
- [ ] Any read-only deployment/configuration finding is documented in the
      implementation handoff; any credential or production action remains an
      explicit operator task.

## Assumptions

- The existing MCP connection test returns failed diagnostics directly to the UI,
  copy action, and debug issue seed. Improving backend diagnostics is therefore
  sufficient for the user-visible behavior.
- The existing `auth_required` result shape and `www_authenticate` field are the
  correct contract for challenged Cloudflare Access redirects.
- Cloudflare Access service credentials and origin bearer credentials are
  intentionally separate factors and must remain independently enforced.
- A credential-free request to `https://vibe.vasandani.dev/mcp` returning an
  Access login redirect means the probe likely did not reach the origin Caddy
  rejection path.
- The repository's documented deployment architecture is two-factor:
  Cloudflare Access service-token headers at the edge plus an origin
  Authorization bearer checked behind Access.
- Shared assignment testing reads each executor's saved native entry, so one
  upstream endpoint failure can correctly appear once per assigned executor.

## Clarifications (resolved)

- Result contract: keep using the existing backend result shape
  (`ok`, `failed`, `auth_required`, `unsupported`, `error`, and
  `www_authenticate`). The settings UI, copy action, and debug issue creation
  already treat `error` as opaque diagnostic text, so backend diagnostics are the
  correct layer for this fix.
- Redirect policy: Streamable HTTP and legacy SSE probes should use a
  no-follow HTTP client policy for the probe requests. A 3xx response is evidence
  to classify or report, not a transport step to hide from the user.
- Authentication classification: existing 401/403 responses remain
  `auth_required` whether or not they include `WWW-Authenticate`. Redirects are
  `auth_required` only when they carry a non-empty, readable
  `WWW-Authenticate` challenge; otherwise they remain `failed`.
- Challenge preservation: a challenged redirect uses the same raw
  `www_authenticate` field as 401/403 because OAuth discovery already consumes
  that field as its first protected-resource metadata hint.
- Redirect diagnostics: the only redirect target detail allowed in `error` is a
  sanitized `Location` indication. For absolute URLs this means scheme, host,
  port when present, and path; for relative URLs this means path only. Query,
  fragment, userinfo, invalid header bytes, and unparseable values are omitted.
- Body diagnostics: successful 2xx responses with non-SSE content types are
  expected to parse as JSON-RPC. If parsing fails or the parsed value is not an
  MCP response for the requested id, the failure diagnostic should add status,
  content type, and a sanitized 200-character preview instead of reporting only
  serde's JSON parser error.
- Secret handling: configured request headers (`headers` / `http_headers`),
  `Authorization`, `CF-Access-Client-Id`, `CF-Access-Client-Secret`, cookies,
  and service/origin bearer values must not appear in diagnostics, logs, copied
  text, or seeded issues as part of this work.
- Deployment interpretation: the documented production architecture remains
  two-factor: Cloudflare Access service-token headers at the edge plus an origin
  bearer behind Access. A credential-free Access redirect is evidence that the
  probe did not reach the origin Caddy 401 path; any credential provisioning or
  production configuration change remains an operator task unless later
  read-only investigation identifies a repository defect.

## Open Questions

- None. All previously implied questions have been resolved against the project
  constitution, prior knowledge, and existing MCP diagnostic/security contracts.
