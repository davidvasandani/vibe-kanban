# Preview leases: trusted-source admission

A public preview lease (`https://<lease-id>.<preview-host>/`) normally needs
the one-time capability link. A deployment can set
`VK_PREVIEW_TRUSTED_SOURCE_CIDRS` so requests from named networks open a live
lease by its plain URL. The code is
`crates/server/src/routes/preview/trusted_source.rs` and
`resolve_public_lease` in `crates/server/src/routes/preview.rs`.

## Design decisions worth keeping

- **The client address is a claim, so its provenance is checked.** It is read
  only from `CF-Connecting-IP`, only when the TCP peer is loopback (the tunnel
  connector), and only when there is exactly one value. `X-Forwarded-For`,
  `X-Real-IP`, `True-Client-IP` and `Forwarded` are never read. Anything
  ambiguous is untrusted and falls back to the capability flow.
- **The preview listener must be served with connect info.** Both
  `crates/server/src/main.rs` and `crates/server/src/startup.rs` serve it with
  `into_make_service_with_connect_info::<SocketAddr>()`. The handler reads
  `ConnectInfo` from request extensions rather than as an extractor, so a
  router served without it fails closed instead of returning 500.
- **Admission replaces one check, not the tail.** The trusted branch only
  chooses the lease (by the ID in the hostname). It then joins the same path as
  the cookie branch, so helper liveness, `Cache-Control: no-store`, reserved
  cookie stripping and the WebSocket lifecycle watcher apply unchanged.
  Workspace, execution and port still come from the stored lease.
- **All leases, not per-lease opt-in.** The agent that creates a lease does not
  know where the person will open it, so a per-lease flag would usually be
  missing when wanted. The operator opt-in is the setting, empty by default.
- **Ranges are bounded.** IPv4 prefixes shorter than `/24` and IPv6 shorter
  than `/48` are rejected, so a typo such as `/0` cannot open every lease.
  Invalid entries are skipped and logged, never widened.
- **`direct_url` is returned only when the setting has a valid entry.** It
  holds no secret. The remote browser is not on a trusted network and keeps
  using the capability `url`.

## Gotchas

- **Lease IDs are not secrets.** They are in hostnames. On a trusted network
  the ID is the only protection, so everyone behind that NAT is admitted.
- **A process on the coordinator can forge the header** by connecting to the
  preview port on loopback. A shared-secret header added at the edge would
  close that, at the cost of an edge rule and a secret to rotate. It was not
  done because such a process already runs on the host holding every
  workspace.
- **Before adding a reverse proxy in front of the preview port, revisit this.**
  A local proxy makes every request's peer loopback and would pass a
  client-supplied `CF-Connecting-IP` straight through.
- **The setting is read once per process.** Changing it needs a restart.
- **Testing leases against an in-memory database.** `preview_leases` has
  foreign keys to workspaces and executions. `SqliteConnectOptions::foreign_keys(false)`
  did not stop enforcement after running migrations; issuing
  `PRAGMA foreign_keys = OFF` on a one-connection pool after migrating did.
- **Admissions log at info once per (lease, address)** and at debug after that,
  because one page load is hundreds of requests.

## Contributed by

- `vk/c5fe-preview-leases-l`
