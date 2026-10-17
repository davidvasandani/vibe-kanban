# Contract: edge and Caddy

## Cloudflare module inputs (`terragrunt/modules/cloudflare-tunnel`)

```hcl
variable "ip_lists" {
  type = map(object({ description = string, cidrs = list(string) }))
  default = {}
}
# hostnames["h"].bypass_access_ip_lists = optional(map(string), {})  # path -> ip_lists key
```
- A `cloudflare_zero_trust_list` (type `IP`) is created per `ip_lists` key,
  named `<tunnel>-<key>`.
- A bypass app whose path is in `bypass_access_ip_lists` gets a single
  bypass policy, `include = [{ ip_list = { id = list.id } }]`.
- Preconditions: every `bypass_access_ip_lists` path is in
  `bypass_access_paths`, is not also in `bypass_access_ip_ranges`, and names
  an existing `ip_lists` key.

## vibe.vasandani.dev bypass set (OpenAI connector list)

`/oauth/mcp`, `/oauth/register`, `/oauth/token`,
`/.well-known/oauth-protected-resource`,
`/.well-known/oauth-authorization-server`.

Never bypassed: `/mcp`, `/oauth`, `/oauth/authorize` (enforced by
`ci/check-edge-exposure.sh`).

## Caddy `:3343` (think2)

```caddy
handle /oauth/mcp* {
  forward_auth 127.0.0.1:3334 {
    uri /api/mcp-oauth/verify
  }
  rewrite * /mcp
  reverse_proxy 127.0.0.1:8787
}
```
Placed before `handle /mcp*`. It never reads `VIBE_MCP_BEARER_TOKEN` or
`Cf-Access-Jwt-Assertion`. Strip `Cf-Access-Jwt-Assertion` before
forward_auth/proxy (`request_header -Cf-Access-Jwt-Assertion`) so nothing
downstream can mistake a forged header for an Access identity.
