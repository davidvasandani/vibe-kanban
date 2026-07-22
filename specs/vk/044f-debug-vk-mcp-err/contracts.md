# Contracts: Debug VK MCP Connection Failure

**Feature dir**: `specs/vk/044f-debug-vk-mcp-err/`
**Task**: `vk/044f-debug-vk-mcp-err`
**Spec**: [`spec.md`](spec.md)

## API Contract

No new HTTP route, database table, or generated TypeScript type is required.
The feature uses the existing MCP test result shape returned by:

- `POST /api/mcp-config/test`
- `POST /api/mcp-config/shared/test`

Existing Rust/TypeScript shape:

```typescript
type McpServerTestResult = {
  name: string;
  transport: 'stdio' | 'http' | 'sse' | 'unknown' | string;
  status: 'ok' | 'failed' | 'auth_required' | 'unsupported';
  latency_ms: number | null;
  tool_count: number | null;
  server_name: string | null;
  server_version: string | null;
  error: string | null;
  www_authenticate: string | null;
};
```

`SharedMcpAssignmentTestResult.result` continues to embed this result without a
schema change. `gateway_status` and `upstream_status` continue to derive from
the same `status` enum.

## Status Classification Contract

HTTP/SSE probe responses classify as follows:

| Condition | `status` | `www_authenticate` | `error` |
| --- | --- | --- | --- |
| Valid MCP handshake over JSON body | `ok` | `null` | `null` |
| Valid MCP handshake over SSE body | `ok` | `null` | `null` |
| HTTP 401 with challenge | `auth_required` | raw challenge | HTTP diagnostic |
| HTTP 401 without challenge | `auth_required` | `null` | HTTP diagnostic |
| HTTP 403 with challenge | `auth_required` | raw challenge | HTTP diagnostic |
| HTTP 403 without challenge | `auth_required` | `null` | HTTP diagnostic |
| HTTP 3xx with usable challenge | `auth_required` | raw challenge | redirect diagnostic |
| HTTP 3xx without usable challenge | `failed` | `null` | redirect diagnostic |
| HTTP 2xx non-MCP JSON body | `failed` | `null` | protocol diagnostic |
| HTTP 2xx non-MCP SSE body | `failed` | `null` | protocol diagnostic |
| HTTP 2xx HTML/plain/other body | `failed` | `null` | protocol diagnostic |
| HTTP 4xx/5xx other than 401/403 | `failed` | `null` | HTTP diagnostic |
| timeout | `failed` | `null` | existing timeout diagnostic |
| connection refused/transport error | `failed` | `null` | existing request diagnostic |
| unsupported config shape | `unsupported` | `null` | existing reason |
| stdio probe success/failure | unchanged | unchanged | unchanged |

The phrase `usable challenge` means a present `WWW-Authenticate` header value
that is non-empty after trimming and can be represented as a UTF-8 string by
the HTTP library.

## Redirect Contract

The MCP probe HTTP client must not automatically follow redirects.

Redirect diagnostics must include:

- HTTP redirect status, such as `HTTP 302`
- safe redirect destination context when a `Location` header is present
- body preview only when the response body is non-empty and safe to include

Redirect diagnostics must not include:

- configured request headers
- `Authorization`
- `Cookie` or `Set-Cookie`
- `CF-Access-Client-Id`
- `CF-Access-Client-Secret`
- query strings or URL fragments from `Location`

Example diagnostic shape:

```text
HTTP 302 redirect to https://example.com/cdn-cgi/access/login
content-type: text/html
body preview: <html>...
```

Exact wording can differ, but the status and safe context must be present.

## Non-MCP 2xx Contract

A successful HTTP status is not sufficient for success. When a 2xx response
cannot be interpreted as a matching MCP JSON-RPC response or MCP SSE response,
the result must be `failed`.

Diagnostics must include:

- HTTP status, usually `HTTP 200`
- `content-type` when present
- a bounded sanitized body preview when present
- the underlying parser/protocol failure in human-readable form

Example diagnostic shape:

```text
HTTP 200 response was not valid MCP JSON
content-type: text/html
body preview: <html><title>Access login</title>...
parse error: expected value at line 1 column 1
```

The result must not be only:

```text
invalid JSON response: expected value at line 1 column 1
```

## Sanitization Contract

Diagnostic helpers must treat response data as untrusted.

Body preview rules:

- Trim leading/trailing whitespace.
- Bound by displayed character count, not bytes; the complete displayed preview
  including any truncation marker must be no more than 200 characters.
- Preserve enough text to identify HTML/login/non-MCP responses.
- Avoid control characters that can disrupt logs or UI.
- Prefer omitting body preview if the body appears credential-heavy and cannot
  be safely redacted.

Header rules:

- Include only allowlisted response facts: status, content type,
  `WWW-Authenticate`, and sanitized redirect location.
- Preserve `WWW-Authenticate` only in the dedicated `www_authenticate` field
  and, if useful, mention challenge presence in the diagnostic.
- Do not echo request headers or arbitrary response headers.

Location rules:

- For absolute URLs, include scheme, host, port when present, and path.
- For relative URLs, include path.
- Remove query and fragment.
- If parsing fails, omit or include a bounded sanitized value with query-like
  content removed.

## Test Contract

Focused tests must be offline and synthetic. They should cover:

- challenged redirect returns `auth_required` and preserves `www_authenticate`
- unchallenged redirect returns `failed` and includes redirect status/context
- redirect `Location` query/fragment secrets are not present in diagnostics
- 2xx HTML response returns `failed` with status, content type, and preview
- 2xx malformed JSON response returns `failed` with status and parse context
- 401/403 auth-required behavior remains unchanged
- valid Streamable HTTP JSON handshake still returns `ok`
- valid Streamable HTTP SSE handshake still returns `ok`
- legacy SSE success still returns `ok`
- legacy SSE 401 remains `auth_required`
- legacy SSE message POST challenged redirect returns `auth_required`
- legacy SSE message POST unchallenged redirect returns `failed`
- stdio bogus command/unsupported/connection-refused behavior remains unchanged

## Deployment Inspection Contract

Any live or deployment inspection is read-only.

Allowed to document:

- response status
- response content type
- redirect destination origin/path
- `WWW-Authenticate` presence and safe value
- whether the response appears to be Cloudflare Access, origin auth, or a
  non-MCP service, as an inference from safe HTTP facts

Not allowed:

- reading, printing, or asking for secrets
- modifying Cloudflare Access, Caddy, tunnel, DNS, service manager, or VK config
- moving Cloudflare Access service credentials into agent-native MCP configs
- adding frontend-only handling for one literal error string
