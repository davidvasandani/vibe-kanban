# Technical spec: trusted-source admission for preview leases

Task: `vk/c5fe-preview-leases-l` — let Camero's WAN IP open
`https://<lease-id>.preview.vasandani.dev/` without the capability link.

## Problem

A preview lease is reachable only through the one-time
`/__vk/open#<capability>` link. The owner wants to open a live lease from
Camero's network by its plain URL. Every other source must keep today's
behaviour.

`*.preview.vasandani.dev` has no Cloudflare Access app, so the gate is entirely
at the origin (`crates/server/src/routes/preview.rs`, `public_preview_request`).
The bypass therefore has to live in that handler.

## Facts established

- Camero's WAN IP is **107.193.224.69** (read from Camero's UDM Pro SE WAN
  health on 2026-11-17: gateway `Camero`, ISP AT&T, `eth9`).
- The WAN is **DHCP, not a static assignment**, and WAN IPv6 is disabled. The
  address has been stable long enough to appear across the homelab repo, but it
  can change; when it does, the homelab value must be updated.
- `70.235.241.95` is the think cluster's egress (confirmed with
  `curl ifconfig.me` from a worker). It is Scott's WAN and must not be trusted
  here. The homelab files that label it "Camero" (and `107.193.224.69` "Scott")
  have the two labels swapped; that is tracked separately.
- cloudflared on think2 reaches the preview listener at `localhost:40776`, so a
  tunnel request has a loopback peer address and carries `CF-Connecting-IP`,
  which Cloudflare sets itself and overwrites if a client sends one.
- The preview listener is served with plain `axum::serve(listener, router)` in
  both `crates/server/src/main.rs` and `crates/server/src/startup.rs`, so the
  handler cannot see the peer address today.

## Decisions on the open questions

1. **Camero's IP**: `107.193.224.69/32`, DHCP-assigned (see above).
2. **All leases, not opt-in.** The goal and acceptance both say any live lease
   opens from Camero. A per-lease opt-in would have to be set by the agent that
   creates the lease, which does not know where the owner will open it, so it
   would usually be missing when wanted. The operator opt-in is the setting
   itself, which is empty by default.
3. **Return the plain URL.** `create_preview_lease` returns `direct_url`
   (`https://<lease-id>.<preview-host>/`) when trusted sources are configured.
   It holds no secret; it only works from a trusted source.

## Design

### Configuration

`VK_PREVIEW_TRUSTED_SOURCE_CIDRS`: comma-separated CIDRs (a bare IP means a
single host). Unset or empty disables the feature. It is parsed once per
process.

Guard rails, so a typo cannot open the origin widely:

- An entry that does not parse is logged at error level and skipped.
- An IPv4 prefix shorter than `/24` or an IPv6 prefix shorter than `/48` is
  rejected the same way. This is "one site", never `0.0.0.0/0`.

### Client address

New module `crates/server/src/routes/preview/trusted_source.rs` (pure
functions, unit-tested):

```
trusted_client_ip(peer: Option<SocketAddr>, headers) -> Option<IpAddr>
```

Returns an address only when all of these hold:

- the TCP peer is known and is loopback (the tunnel connector);
- there is exactly one `CF-Connecting-IP` header and it parses as an IP.

`X-Forwarded-For`, `X-Real-IP`, `Forwarded` and `True-Client-IP` are never
read. IPv4-mapped IPv6 addresses are canonicalised before matching.

The preview listener is served with
`into_make_service_with_connect_info::<SocketAddr>()` in both entrypoints so
the handler receives `ConnectInfo<SocketAddr>`. The handler takes it as an
`Option`, so a router served without connect info fails closed.

### Authorization

In `public_preview_request`, after the `/__vk/open` and `/__vk/authorize`
branches (unchanged):

1. If the request has a trusted client address inside a configured CIDR, load
   the lease by the hostname's lease ID.
   - no such lease: `404`;
   - expired or revoked: `410`;
   - otherwise continue with that lease.
2. Otherwise run the existing cookie path, unchanged.

Both paths then share the existing tail: the helper process must be running,
and the request is proxied on the lease's stored workspace, execution and port
with `lease_guard = Some(lease)`. That keeps `Cache-Control: no-store`, the
reserved-cookie stripping in both directions and the WebSocket lifecycle
watcher on the trusted path with no duplicated code. A trusted source selects
nothing: host, port and workspace all come from the stored lease.

### Logging

A trusted admission logs `preview lease admitted from trusted source` with the
lease ID and client IP at info level the first time a (lease, IP) pair is seen
in this process, and at debug level afterwards, so a page load with hundreds of
assets does not flood the log. The seen-set is bounded.

### API and MCP

- Server `CreatePreviewLeaseResponse` gains `direct_url: Option<String>`, set
  when at least one valid trusted CIDR is configured.
- The MCP `create_preview_lease` result passes it through and its description
  says what it is for.

### Homelab (separate repo and PR)

- `modules/vibe-kanban-rebuild.nix`: new option
  `previewTrustedSourceCidrs` (default `[ ]`), exported as
  `VK_PREVIEW_TRUSTED_SOURCE_CIDRS` next to `VK_PUBLIC_PREVIEW_URL`.
- `hosts/think/think2.nix`: set it to `[ "107.193.224.69/32" ]`.
- `terragrunt/.../cloudflare-tunnel-vibe-remote/terragrunt.hcl`: extend the
  `edge-open-ok` justification with the trusted-source contract (comment only).
- `ci/check-edge-exposure.sh`: require that contract text, and require the
  think2 trusted list to be exactly the reviewed set, which rejects
  `70.235.241.95`.

The env var is ignored by a VK build that predates this change, so the two PRs
can land in either order.

## Security trade-off

For requests from Camero's WAN address, protection changes from "holds the
256-bit capability" to "is behind Camero's NAT and knows a live lease ID".
Lease IDs are random UUIDs but are not secret: they appear in hostnames. Any
device or guest on Camero's network gets this access for as long as a lease is
live (at most four hours).

Residual trust in the client address:

- A process on the coordinator host can connect to `localhost:40776` and forge
  `CF-Connecting-IP`. Such a process is already inside the trust boundary of
  the host that holds every workspace, and still needs a live lease ID.
- If Camero's DHCP address is reassigned by the ISP, the new holder would be
  trusted until the homelab value is updated.

## Tests

Unit tests in `trusted_source.rs`:

- CIDR parsing: valid, bare IP, invalid, too-broad, empty.
- trusted: loopback peer + in-range `CF-Connecting-IP`;
- untrusted: loopback peer + out-of-range address (`70.235.241.95`);
- spoofed: non-loopback peer with the header; `X-Forwarded-For` only;
  duplicate `CF-Connecting-IP`; missing peer.

Handler tests in `preview.rs` against a real test deployment where the existing
test helpers allow it: trusted source with a live lease reaches the proxy tail;
expired and revoked leases return `410`; unknown lease returns `404`;
untrusted source without a cookie behaves as before.

Homelab: `ci/check-edge-exposure.sh` passes, and fails when the trusted list is
changed to `70.235.241.95/32`.

## Out of scope

- Fixing the swapped Camero/Scott labels elsewhere in the homelab repo.
- Any Cloudflare edge rule for the preview hostname.
