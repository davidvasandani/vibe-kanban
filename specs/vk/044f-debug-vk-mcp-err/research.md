# Research: Debug VK MCP Connection Failure

**Feature dir**: `specs/vk/044f-debug-vk-mcp-err/`
**Task**: `vk/044f-debug-vk-mcp-err`
**Spec**: [`spec.md`](spec.md)

## Sources Reviewed

- [`spec.md`](spec.md)
- Repository root [`IMPLEMENTATION_PLAN.md`](../../../IMPLEMENTATION_PLAN.md)
- Repository root [`PRIOR_KNOWLEDGE.md`](../../../PRIOR_KNOWLEDGE.md)
- [`crates/mcp/AGENTS.md`](../../../crates/mcp/AGENTS.md)
- [`crates/executors/src/mcp_test.rs`](../../../crates/executors/src/mcp_test.rs)
- [`crates/executors/src/mcp_oauth.rs`](../../../crates/executors/src/mcp_oauth.rs)
- [`crates/server/src/routes/config.rs`](../../../crates/server/src/routes/config.rs)
- [`crates/server/src/mcp_gateway/mod.rs`](../../../crates/server/src/mcp_gateway/mod.rs)
- [`crates/server/src/mcp_gateway/proxy.rs`](../../../crates/server/src/mcp_gateway/proxy.rs)
- Prior VK MCP specs:
  - [`specs/vk/76d1-vk-mcp-ux`](../76d1-vk-mcp-ux/)
  - [`specs/vk/9453-vk-mcp-auto-debu`](../9453-vk-mcp-auto-debu/)

Note: the root implementation and prior-knowledge notes currently describe an
AWS SSO feature, not this MCP diagnostic task. They were still reviewed for
repository planning style and reusable process constraints: keep process
boundaries explicit, avoid credential leakage, add focused tests, regenerate
types only when contracts change, and finish with formatting/checks.

## Existing Probe Shape

`crates/executors/src/mcp_test.rs` owns the connection probe used by MCP
settings. It normalizes untyped agent config into:

- `McpProbeTarget::Stdio`
- `McpProbeTarget::Http`
- `McpProbeTarget::Sse`
- `McpProbeTarget::Unsupported`

`test_mcp_servers()` creates a single `reqwest::Client::new()` and runs probes
concurrently. The result contract is:

- `status`: `ok`, `failed`, `auth_required`, or `unsupported`
- `error`: optional opaque diagnostic text
- `www_authenticate`: optional raw challenge, currently populated for 401/403
  HTTP/SSE responses

Decision: keep this result shape. The UI and shared test route already consume
`auth_required`, `error`, and `www_authenticate`, so this feature should improve
classification and diagnostic content without new generated types.

## Redirect Behavior

The current probe uses `reqwest::Client::new()`, whose default behavior follows
redirects. That hides intermediate 3xx responses, including Access redirects,
and can leave the final HTML login body to be parsed as JSON. That produces the
opaque error named in the spec:

```text
invalid JSON response: expected value at line 1 column 1
```

Other code in the repo already disables redirects when the redirect itself is
security-relevant:

- `crates/executors/src/mcp_oauth.rs` uses `redirect(Policy::none())` so a
  validated URL cannot bounce elsewhere during OAuth discovery.
- `crates/server/src/mcp_gateway/proxy.rs` uses `redirect(Policy::none())` for
  gateway upstream forwarding.
- `crates/services/src/services/aws_sso.rs` also uses a no-redirect client for
  an auth probe.

Decision: build the MCP test HTTP client with `redirect(reqwest::redirect::Policy::none())`.
Redirects should then pass through the same status-classification layer as
other non-success responses.

## Authentication Classification

Current `http_status_error()` classifies only HTTP 401 and 403 as
`AuthRequired`, and preserves `WWW-Authenticate`. The feature spec requires
challenged redirects to become `auth_required` too.

Decision: generalize HTTP response classification:

- 401 and 403 remain `auth_required`, with any challenge retained.
- 3xx responses with a non-empty, syntactically usable `WWW-Authenticate`
  challenge become `auth_required`, preserving the challenge.
- 3xx responses without such a challenge remain `failed`.
- Other non-success responses remain `failed`.

The implementation does not need to parse Cloudflare Access deeply. It should
recognize the presence of a usable challenge and pass the raw challenge through
the existing field for the OAuth/connect flow.

## Diagnostic Content

Current non-success diagnostics are:

```rust
format!("HTTP {}: {}", status.as_u16(), snippet(&text))
```

For 2xx non-MCP responses, JSON parsing errors currently discard HTTP context:
the code reads the body and returns only `invalid JSON response: {e}`. For SSE,
the probe can report generic event-stream failures without status/content-type
context once the initial status is 2xx.

Decision: add a bounded safe diagnostic builder for HTTP probe responses. It
should include only allowlisted facts:

- HTTP status code
- content type, when present
- redirect destination context, when present
- sanitized response body preview, when useful

Do not echo request headers. Do not dump arbitrary response headers. Use body
preview limits appropriate for UI display and issue seeding.

## Sanitization

Risky diagnostic sources:

- `Location` may contain secrets in query params.
- HTML login bodies may include opaque tokens, callback URLs, cookies, or
  form fields.
- Configured request headers may include `Authorization`,
  `CF-Access-Client-Id`, `CF-Access-Client-Secret`, cookies, and other secrets.

Decision: sanitize by omission and allowlisting:

- Do not include configured request headers at all.
- Do not include `Set-Cookie`, `Cookie`, `Authorization`, or Cloudflare
  service-token headers.
- For redirect locations, include origin/path when parsable, and redact query
  and fragment. If unparsable, include only a sanitized bounded preview or omit.
- For body previews, strip control characters, normalize whitespace enough for
  display, bound by character count, and redact obvious credential-bearing
  fields if a local helper already supports that. Prefer omission over broad
  regex guessing when uncertain.

## Existing Tests

`mcp_test.rs` already contains unit and async tests for:

- config normalization for stdio, HTTP, SSE, Gemini `httpUrl`, Codex
  `http_headers`, disabled servers, and unsupported shapes
- JSON-RPC stdio handshake over in-memory duplex
- bogus stdio command failure
- HTTP 401 with `WWW-Authenticate`
- HTTP 403 without `WWW-Authenticate`
- HTTP 500
- SSE 401
- connection refused

It also has a small `one_shot_http_server()` fixture that is enough for
first-request failure cases.

Decision: extend these in-place. For multi-step HTTP/SSE success tests, add a
slightly richer loopback fixture that can serve a sequence of canned responses
or a small axum/hyper test server if the existing dependencies make that
cleaner. Do not require external network access. Cover both the initial legacy
SSE GET path and the legacy SSE message POST path, because redirects can occur
on either request.

## Gateway And Deployment Boundary

The shared MCP route in `crates/server/src/routes/config.rs` tests gateway
managed URLs separately from native URLs and maps result status into
`gateway_status`/`upstream_status` strings. No separate frontend or route
contract is needed if the underlying `McpServerTestResult` is improved.

The gateway path stores two independent credential classes:

- origin OAuth/bearer token, forwarded as `Authorization: Bearer ...`
- optional Cloudflare Access service-token headers, forwarded only when stored

`mcp_gateway/proxy.rs` forwards only an allowlist of MCP protocol headers from
the local client, replaces Authorization with the stored upstream token, and
adds Cloudflare Access credentials only from stored gateway credentials.

Decision: diagnostics must preserve that security boundary. The probe can
report "authentication required" and the challenge header, but must not suggest
moving Cloudflare Access service-token values into regular native MCP configs.

## Read-only Live Endpoint Inspection

The spec asks implementation to inspect the live endpoint and repository
deployment configuration read-only. This planning pass did not perform network
inspection because the Codex environment has restricted network access and the
task request was to plan, not implement.

Decision for implementation handoff:

- Use synthetic tests as the authoritative regression coverage.
- If network is available during implementation, perform a credential-free
  `curl -i -L=false` style check or equivalent against
  `https://vibe.vasandani.dev/mcp` and document only safe facts: status,
  content type, redirect target origin/path, and `WWW-Authenticate` presence.
- Do not read, print, or request Cloudflare Access service-token secrets or
  origin bearer tokens.
