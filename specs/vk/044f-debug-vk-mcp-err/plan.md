# Technical Plan: Debug VK MCP Connection Failure

**Feature dir**: `specs/vk/044f-debug-vk-mcp-err/`
**Task**: `vk/044f-debug-vk-mcp-err`
**Spec**: [`spec.md`](spec.md)

## Approach

Fix the diagnostic at the backend probe boundary in
`crates/executors/src/mcp_test.rs`. The settings UI already renders the
backend `McpServerTestResult.error` and uses `auth_required` plus
`www_authenticate` for Connect/OAuth. The shared test route in
`crates/server/src/routes/config.rs` also passes the result through unchanged.

The implementation should therefore:

- make HTTP redirects visible by using a no-redirect `reqwest` client for MCP
  tests;
- classify challenged redirects as `auth_required`;
- enrich failed HTTP/SSE diagnostics with safe status/content-type/preview
  context;
- preserve all existing success behavior for stdio, Streamable HTTP JSON,
  Streamable HTTP SSE, and legacy SSE.

No frontend contract, database schema, API route, or generated TypeScript type
change is expected.

## Grounding

- `crates/executors/src/mcp_test.rs`
  - `test_mcp_servers()` creates the HTTP client with `reqwest::Client::new()`.
  - `http_status_error()` classifies 401/403 and builds current HTTP errors.
  - `http_send()` parses successful HTTP JSON/SSE bodies and currently loses
    HTTP context on JSON parse failures.
  - `probe_sse()` performs the legacy SSE GET and then POSTs messages to the
    announced endpoint.
  - Existing tests already include loopback one-shot HTTP fixtures.
- `crates/server/src/routes/config.rs`
  - `POST /api/mcp-config/test` returns `Vec<McpServerTestResult>`.
  - `POST /api/mcp-config/shared/test` embeds the same result in
    `SharedMcpAssignmentTestResult`.
  - Shared gateway/native probes already map the enum status into UI-facing
    gateway/upstream statuses.
- `crates/executors/src/mcp_oauth.rs`
  - OAuth HTTP client disables redirects and uses `WWW-Authenticate` as the
    discovery hint.
- `crates/server/src/mcp_gateway/proxy.rs`
  - Gateway forwarding disables redirects and preserves the boundary between
    origin bearer credentials and Cloudflare Access service credentials.
- `packages/web-core/src/shared/dialogs/settings/settings/McpSettingsSection.tsx`
  - Already displays full failed diagnostics from `result.error` and supports
    debug-issue/copy flows from prior MCP auto-debug work.

## Implementation Steps

1. Establish baseline.
   - Run the focused existing MCP probe tests:
     `cargo test -p executors mcp_test`.
   - Optionally run `cargo test -p server config::` if route tests are relevant
     after changes.
2. Replace MCP test client construction.
   - In `test_mcp_servers()`, build the client with
     `reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())`.
   - If client construction unexpectedly fails, preserve unsupported config
     results by normalizing each entry first, then report builder failure only
     for runnable HTTP/SSE/stdio targets that would otherwise have been probed.
   - Keep default timeout behavior controlled by the existing outer
     `tokio::time::timeout`; do not add broad new client policy unless needed.
3. Introduce small HTTP diagnostic helpers in `mcp_test.rs`.
   - `response_content_type(headers) -> Option<String>`
   - `response_www_authenticate(headers) -> Option<String>`
   - `safe_location(headers, base_url?) -> Option<String>`
   - `safe_body_preview(text) -> Option<String>`
   - `http_failure_diagnostic(status, content_type, location, body_preview, parse_error?)`
   Keep helpers private and unit-testable.
4. Update non-success response handling.
   - Replace or extend `http_status_error(resp)` so it preserves:
     status, content type, sanitized `Location`, bounded body preview, and
     `WWW-Authenticate`.
   - Return `ProbeError::AuthRequired` for 401/403 as today.
   - Return `ProbeError::AuthRequired` for 3xx when
     `WWW-Authenticate` is present and non-empty.
   - Return `ProbeError::Other` for unchallenged redirects and other errors.
5. Update Streamable HTTP 2xx parsing diagnostics.
   - In `http_send()`, capture status and content type before consuming the
     body.
   - For JSON-path failures, include HTTP status, content type, safe preview,
     and parse/matching-id/extract-result context.
   - Preserve current successful JSON behavior, including batch response
     matching and `Mcp-Session-Id`.
   - Preserve current successful SSE response behavior.
6. Update legacy SSE diagnostics.
   - On initial GET non-success, use the shared HTTP response classifier.
   - On message POST non-success, use the same classifier.
   - For a 2xx response that is not a usable event stream, report status and
     content type/body context where available without breaking valid SSE.
7. Add or extend offline tests in `mcp_test.rs`.
   - Add a sequence-capable loopback fixture for multi-request Streamable HTTP
     and legacy SSE success paths if the one-shot helper is insufficient.
   - Cover challenged redirect:
     `302 Location: ...` plus `WWW-Authenticate: Cloudflare-Access ...`
     returns `auth_required` and preserves the header.
   - Cover unchallenged redirect returns `failed`.
   - Cover `Location` query/fragment redaction.
   - Cover 2xx HTML/non-MCP JSON parse diagnostics.
   - Cover valid Streamable HTTP JSON success.
   - Cover valid Streamable HTTP SSE success.
   - Cover valid legacy SSE success.
   - Cover legacy SSE message POST redirects after the endpoint event for both
     challenged and unchallenged redirects.
   - Keep existing 401/403, connection-refused, unsupported, and stdio tests
     passing.
8. Consider route-level coverage only if behavior is not fully proven at the
   executor crate boundary.
   - The config routes mostly pass through probe results. A route test is useful
     only if shared gateway/native result mapping changes.
9. Read-only deployment/config inspection during implementation handoff.
   - If network and repository deployment config access are available, inspect
     the live endpoint without credentials and document only safe facts:
     status, content type, sanitized redirect target, and challenge presence.
   - Do not require live network access for tests or correctness.
10. Run validation.
    - `cargo test -p executors mcp_test`
    - `cargo test -p server mcp_auth` if OAuth/challenge behavior is touched
    - `pnpm run backend:check`
    - `pnpm run check`
    - `pnpm run lint`
    - `pnpm run format`
    - `git diff --check`
11. Perform an independent diff review before completion.
    - Confirm no generated files were edited manually.
    - Confirm no request headers or credential values appear in diagnostics.
    - Confirm UI/debug issue surfaces receive richer backend diagnostics
      without frontend special-casing.

## Contracts

See [`contracts.md`](contracts.md). The existing `McpServerTestResult` contract
is retained; this feature changes classification and diagnostic content only.

## Data Model

No persistent data model change.

Transient implementation-only data may include a private diagnostic struct in
`mcp_test.rs`, for example:

```rust
struct HttpDiagnostic {
    status: reqwest::StatusCode,
    content_type: Option<String>,
    location: Option<String>,
    body_preview: Option<String>,
}
```

Do not export this type or derive `TS`; it is only a helper for building
existing `error: Option<String>` values.

## Constitution Check

- **I Clarity over cleverness**: classify redirects/non-MCP bodies explicitly at
  the probe boundary.
- **II Test the contract**: use synthetic loopback HTTP/SSE fixtures to prove
  each status and diagnostic contract offline.
- **III Small, reversible steps**: one backend probe module should absorb most
  changes; routes and UI stay unchanged unless tests prove otherwise.
- **IV Shared-component boundaries**: no shared frontend component changes are
  expected.
- **V Remote mutations**: no remote mutation path is added or changed.
- **VI Don't rebuild what shipped**: keep the hand-rolled minimal MCP probe and
  existing OAuth/connect flow.
- **XI Diagnostics are evidence, not decoration**: diagnostics include safe HTTP
  evidence while omitting credentials and arbitrary headers.

## Risks

- `reqwest` default redirect behavior is currently shared by all HTTP/SSE MCP
  tests. Disabling redirects may reveal statuses callers never saw before; tests
  must lock the intended classification.
- A `WWW-Authenticate` header on a redirect is unusual but valid for the target
  failure mode. The implementation should not treat every redirect as
  OAuth-capable.
- Reading a response body consumes it. Helpers must capture headers before body
  consumption and avoid double reads.
- SSE streams can be long-lived. Do not buffer a successful event stream just
  to build diagnostics; only read bounded previews on failure paths where the
  body is being consumed anyway.
- Sanitizing URLs by string manipulation can leak query secrets. Prefer
  `reqwest::Url` parsing and explicit query/fragment removal.
- The live endpoint may have a separate deployment defect in addition to probe
  behavior. Keep live inspection read-only and document it separately from the
  code fix.

## Rollback

Revert the private helpers, redirect-policy client construction, diagnostic
format changes, and added tests in `crates/executors/src/mcp_test.rs`. Because
the API shape and persisted data remain unchanged, rollback is a backend probe
revert only.
