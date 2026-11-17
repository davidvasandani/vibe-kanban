# Prior knowledge

Searched `vibe-kanban/wiki/` and `homelab/docs/knowledge-base/` for preview
leases, edge exposure, trusted sources, `CF-Connecting-IP` and peer-address
handling. The vibe-kanban wiki has no page on preview leases (its one "preview"
page, `preview-app-url-persistence.md`, is about the embedded iframe preview
and does not apply). The relevant pages are in the homelab knowledge base.

## homelab `remote-browser-preview-leases.md` (`vk/76f7-enable-remote-fi`)

- A lease binds one workspace, one helper execution, one unprivileged port and
  a four-hour deadline. The port comes only from the stored lease. The trusted
  path must keep this: it may pick a lease by hostname but nothing else.
- Authorization checks the helper's live status on every request, and
  established WebSockets need the lifecycle watcher or they outlive revocation.
  The trusted path must go through the same tail (`lease_guard = Some(lease)`).
- Capability-protected responses must be `Cache-Control: no-store`, otherwise a
  shared edge cache can serve content after revocation. This matters more on
  the trusted path, where a cached response would reach untrusted sources.
- The nested wildcard needs its own certificate and full-FQDN DNS record.
  Nothing here changes that.

## homelab `cloudflare-access-trusted-source-bypasses.md` (`vk/7327-camero-ip-can-by`)

- Trusted sites are pinned as exact `/32`s, and the CI check compares the exact
  set (containment or prefix tests are not enough). Reuse that for the think2
  trusted list.
- State the risks where the bypass is documented: anyone behind the shared WAN
  address is admitted, and an ISP re-lease changes who holds the address.
  Re-verify the address from that network before changing the CIDR.
- Repo checks prove declared state only. Live verification from the trusted
  network is separate and must be reported as not run if it was not run.
- This page and its checks carry the swapped labels (`70.235.241.95` as
  Camero). This task does not rely on those labels; the IP was read from
  Camero's gateway.

## homelab `public-booking-page-anonymous-writes-and-dst.md`

- Precedent for using `CF-Connecting-IP` at the origin of a tunnel-fronted
  host, and for the fact that `check-edge-exposure.sh` only knows a fully open
  host through its `edge-open-ok` marker, so the contract text must be pinned.

## In the codebase

- `crates/server/src/mcp_gateway/mod.rs` already gates a route on the TCP peer
  with `ConnectInfo<SocketAddr>` and `peer.is_loopback()`. The main listener is
  served with `into_make_service_with_connect_info`; the preview listener is
  not yet.
- `ipnet` is already in `Cargo.lock` as a transitive dependency.
