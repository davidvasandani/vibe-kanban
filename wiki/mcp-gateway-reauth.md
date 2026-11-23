# MCP gateway credential renewal

Gateway auto-auth targets use `mcp-gateway:<unique-server-name>` with UUID fallback for ambiguous or unsafe names. UUID aliases resolve to the same canonical run. Inventory is restricted to the current owner/machine and connected OAuth entries, including OAuth behind Cloudflare service-token Access.

## Separate the agent capability from the upstream grant

The bearer in an executor MCP configuration is a stable gateway capability. Renewal updates only the encrypted upstream OAuth grant and its expiry/version. Rewriting `.claude.json` or minting another capability cannot repair an expired upstream token. Existing callers immediately use the new grant; a client which removed tools after initialization failure may still need tool refresh or, for Claude Code, a session restart.

Proxy and auto-auth renewal share a per-entry lock, reload credentials, and persist with a version/status/owner/machine compare-and-swap. A disconnect or reconnect must not be overwritten, and credentials from a reconnect to another URL must never be returned to an old proxy request.

## State and proactive renewal

Discovery uses bounded MCP `initialize` probes (12 seconds each, eight concurrent, 30 seconds overall). A successful protocol response proves authenticated state; an arbitrary HTTP 200 does not. HTTP 401 or explicit `token_expired`/`invalid_token` on 403 indicates rejected credentials. Transport failures, unrelated forbidden responses and protocol errors remain unknown and do not trigger the sweep.

Stored `expires_in`, or JWT `exp` as a scheduling hint, selects authenticated entries due within the sweep interval plus five minutes. The normal sweep interval is 30 minutes. Refreshable grants within one minute of expiry are also refreshed before proxying a request. Opaque tokens without expiry can only be checked reactively. Token refresh retains an existing refresh token when the response omits a replacement.

## Browser fallback and refusal scope

Use the refresh grant first. Missing or explicitly rejected refresh grants can fall back to the shared `vk-entra` browser profile; token endpoint outages cannot. Preserve the original callback URI and granted scopes. Older records infer the callback from the configured public OAuth base or local server port.

BrowserSession execute calls have isolated route-handler lifetimes. Callback interception, navigation and waiting must happen in one execute call, with route cleanup in finally. Check exact callback origin/path and unique matching state/code before accepting an authorization code or classifying callback errors. Close the gateway browser session before warming the shared Entra session to avoid profile-lock contention.

Consent is a target-specific refusal. Definitive Entra account refusals from state-validated callbacks or text observed on HTTPS login.microsoftonline.com use the shared account refusal latch. Acquire that latch only for browser fallback: an unrelated Entra refusal must not block refresh-token renewal. Unknown pages/timeouts are failures, not evidence that human action is required.

## Cloudflare and recovery diagnostics

An upstream Cloudflare OAuth `invalid_token` follows the same refresh/browser path. A Cloudflare Access policy rejection or expired Access service-token pair is a separate credential problem and is not repaired by OAuth renewal. Keep unrelated 403 responses unknown rather than repeatedly starting SSO.

Gateway JSON authentication errors include `reauth_target` while preserving upstream diagnostics. Agents should list targets, invoke that target, poll its run, and then refresh missing tools or restart the client session. The `sgsc:dp` and `sgsc:sf` backend targets repair different credentials.

## Verification boundary

The regression fixture forces expiry, concurrently refreshes the entry, verifies initialize succeeds, and asserts that the agent capability hash remains unchanged. It also covers stale writes, disconnects, cross-upstream reconnects, retained refresh grants, and transport classification. Browser interception has mock coverage; this is not evidence of a live provider SSO success.

## Contributed by

- `vk/9784-auto-auth-renew`
