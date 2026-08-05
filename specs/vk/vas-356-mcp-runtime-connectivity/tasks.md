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
