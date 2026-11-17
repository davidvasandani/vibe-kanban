# Implementation plan

Builds on `technical-spec.md` and `prior-knowledge.md`.

## vibe-kanban

1. **Dependency.** Add `ipnet = "2"` to `crates/server/Cargo.toml` (already in
   `Cargo.lock` transitively).

2. **`crates/server/src/routes/preview/trusted_source.rs`** (new; `preview.rs`
   declares `mod trusted_source;`, which resolves to that path).
   - `TRUSTED_SOURCE_CIDRS_ENV = "VK_PREVIEW_TRUSTED_SOURCE_CIDRS"`.
   - `parse_trusted_cidrs(&str) -> Vec<IpNet>`: split on commas, trim, accept a
     bare IP as a host network, skip and log invalid or too-broad entries
     (IPv4 shorter than /24, IPv6 shorter than /48), normalise with `trunc()`.
   - `configured_cidrs() -> &'static [IpNet]` backed by a `LazyLock` that reads
     the environment once.
   - `trusted_client_ip(peer: Option<SocketAddr>, &HeaderMap) -> Option<IpAddr>`:
     loopback peer and exactly one parseable `CF-Connecting-IP`; canonicalise.
   - `trusted_source(peer, headers, cidrs) -> Option<IpAddr>`: the above plus
     CIDR membership.
   - `note_admission(lease_id, ip)`: info on first sighting, debug after;
     bounded seen-set.
   - Unit tests for every case in the spec.

3. **`crates/server/src/routes/preview.rs`.**
   - `subdomain_proxy_request` takes
     `Option<ConnectInfo<SocketAddr>>` and passes the peer to
     `public_preview_request`.
   - Extract the lease lookup into
     `resolve_public_lease(pool, host_lease_id, trusted_ip, cookie, path)
      -> Result<PreviewLease, Response>` so it can be tested against an
     in-memory database without a full deployment. Trusted branch first
     (lookup by ID: 404 unknown, 410 expired/revoked), then the unchanged cookie
     branch.
   - The shared tail (process running, `proxy_cluster_preview_on_port` with
     `Some(lease)`) is untouched.
   - Tests: trusted live lease resolves; trusted expired and revoked return
     410; trusted unknown returns 404; untrusted without cookie returns the
     bootstrap page on `/` and 401 elsewhere; untrusted with the right cookie
     still resolves; a trusted source cannot open lease A through host B's
     cookie-less request for an unknown ID.

4. **Listeners.** In `crates/server/src/main.rs` and
   `crates/server/src/startup.rs`, serve the preview router with
   `into_make_service_with_connect_info::<SocketAddr>()`.

5. **Create response.** `crates/server/src/routes/workspaces/execution.rs`:
   add `direct_url: Option<String>` to `CreatePreviewLeaseResponse`, filled
   when `configured_cidrs()` is non-empty. Unit-test the URL builder.

6. **MCP.** `crates/mcp/src/task_server/tools/preview_leases.rs`: add the
   optional `direct_url` field (skipped when absent) and extend the tool
   description.

7. **Docs.** `docs/remote-browser-dev-server-verification.mdx`: document the
   setting, the `direct_url` field and the changed security boundary.
   `vibe-kanban.env.example`: add the variable if that file lists the
   neighbouring preview settings.

8. **Verify.** `cargo test -p server preview`, `cargo test -p mcp`,
   `pnpm run generate-types:check`, `pnpm run format`,
   `cargo clippy -p server -p mcp`.

## homelab (own branch and PR)

9. `modules/vibe-kanban-rebuild.nix`: option `previewTrustedSourceCidrs`
   (list of strings, default empty) and the env export, omitted when empty.
10. `hosts/think/think2.nix`: `previewTrustedSourceCidrs = [ "107.193.224.69/32" ]`
    with a comment naming Camero, the DHCP caveat and how to re-verify.
11. `terragrunt/environments/cloudflare-tunnel-vibe-remote/terragrunt.hcl`:
    extend the `edge-open-ok` comment for `*.preview.vasandani.dev`.
12. `ci/check-edge-exposure.sh`: require the trusted-source contract text in the
    host block, and require the think2 list to equal the reviewed set. Check
    the repository's protected-path rules for this file before editing.
13. Update `docs/knowledge-base/remote-browser-preview-leases.md` in the
    knowledge stage.
14. Verify with `bash ci/check-edge-exposure.sh`, a mutation to
    `70.235.241.95/32` that must fail, and a Nix evaluation of the think2
    environment value.

## Order and rollout

The VK change is inert until the variable is set, and the variable is ignored
by an older VK build, so either PR can merge first. Live verification from
Camero's network needs both deployed and a device at Camero; if that cannot be
run from here it is reported as not run.
