# Tasks: Cluster-safe MCP runtime connectivity

## Layer 1 — configuration changes

- [x] T001 Add `VIBE_BACKEND_URL` to the VK worker unit, derived from
      `worker.coordinatorUrl`.
- [x] T002 [P] Split think1 protected-port rules and allow think3/think4 on
      Firecrawl TCP 3410 only.

## Layer 2 — contract verification

- [x] T003 Add or extend focused homelab checks for the worker environment.
- [x] T004 [P] Add or extend focused homelab checks for exact nftables scope.

## Layer 3 — validation

- [x] T005 Run formatting and focused Nix evaluation/checks.
- [x] T006 Run independent Codex review and address confirmed findings.

## Layer 4 — documentation and delivery

- [x] T007 Update reusable knowledge and its index.
- [x] T008 Commit repository changes separately.
- [x] T009 Publish and merge PRs in dependency-safe order.

## Follow-up layer 1 — protocol and reusable adapter

- [x] T010 Add optional bounded `McpConfigSnapshot` to
      `crates/cluster-protocol/src/lib.rs` and update protocol fixtures.
- [x] T011 [P] Add MCP server-map extraction and atomic replacement helpers to
      `crates/executors/src/mcp_config.rs` with preservation tests.

## Follow-up layer 2 — coordinator and worker

- [x] T012 Attach the selected executor's snapshot and digest it in
      `crates/local-deployment/src/container.rs`.
- [x] T013 Apply validated snapshots before job creation/spawn in
      `crates/worker/src/execution.rs`; add mismatch, oversize, absence, and
      secret-safe error tests.

## Follow-up layer 3 — deployment and verification

- [x] T014 Remove the duplicate Firecrawl MCP seed from
      `../homelab/modules/vibe-kanban-rebuild.nix` and update its invariant.
- [x] T015 Run formatting and focused protocol/executor/worker/container tests.
- [x] T016 Run independent Codex review and resolve significant findings.

## Follow-up layer 4 — knowledge and delivery

- [x] T017 Update `docs/knowledge-base/cluster-mcp-runtime-connectivity.md` and
      `docs/knowledge-base/INDEX.md`.
- [x] T018 Commit, publish, and merge Vibe Kanban and homelab PRs in rollout
      order.
