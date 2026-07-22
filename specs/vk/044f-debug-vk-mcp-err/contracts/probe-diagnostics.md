# Contract: MCP probe HTTP classification and diagnostics

## Classification

| Observed response | Status | `www_authenticate` |
| --- | --- | --- |
| Valid MCP JSON or SSE handshake | `ok` | absent |
| HTTP 401 or 403 | `auth_required` | exact valid header when present |
| HTTP 3xx with non-empty `WWW-Authenticate` | `auth_required` | exact header |
| HTTP 3xx without challenge | `failed` | absent |
| HTTP 2xx with invalid/non-MCP payload | `failed` | absent |
| Other non-success HTTP status | `failed` | absent |

The probe does not follow HTTP redirects. The configured endpoint is the only
endpoint contacted for the request that produced the classification.

## Diagnostic content

- Non-success: `HTTP <code>` plus a bounded body preview; redirects may add a
  sanitized destination containing scheme, host, optional port, and path only.
  HTML-like bodies use the omission marker at every status.
- Invalid 2xx: `HTTP <code>`, response content type when present, JSON parsing
  context, and a maximum 200-character trimmed body preview. HTML-like bodies
  are represented by an omission marker because login pages can embed secrets
  that cannot be reliably redacted.
- Legacy SSE 2xx responses with an explicit non-SSE content type report HTTP
  status/content type and the same safe preview/omission rule instead of a
  generic closed-stream error.
- Never include configured request headers, Authorization values,
  `CF-Access-Client-*` values, cookies, redirect query/fragment/userinfo, or
  unbounded content.

## Compatibility

No API shape changes. `POST /api/mcp-config/test` and
`POST /api/mcp-config/shared/test` continue returning existing
`McpServerTestResult` and `SharedMcpAssignmentTestResult` objects. Existing UI
display/copy/debug behavior consumes the improved `status`, `error`, and
`www_authenticate` fields unchanged.
