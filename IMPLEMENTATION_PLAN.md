# Implementation Plan: Provide Bubblewrap to Cluster Worker Agents

Task id: `8475-bubblewrap-missi`

1. Refresh the project constitution with the service-only scope, execution
   boundary, least-exposure, and verification requirements that govern this
   change.
2. Create the SpecKit feature directory and specification for the worker
   bubblewrap prerequisite, then resolve any remaining ambiguity.
3. Produce SpecKit research and technical plan artifacts describing the NixOS
   systemd-path mechanism and the available module evaluation strategy.
4. Generate a dependency-ordered task list and analyze the artifacts for scope,
   test, and constitution gaps.
5. Add `pkgs.bubblewrap` to
   `systemd.services.vibe-kanban-worker.path` in
   `homelab/modules/vibe-kanban-rebuild.nix`.
6. Add or update the narrowest existing Nix regression check that evaluates the
   worker unit's executable path and proves `bwrap` is present.
7. Run focused checks, Nix evaluation, and diff inspection without deploying or
   switching a host.
8. Run an independent Codex diff review; address confirmed significant
   findings and repeat verification/review until clean.
9. Record the reusable worker-unit dependency rule in the project knowledge
   base, update its index with task id `8475-bubblewrap-missi`, and commit the
   knowledge-base update separately before handoff.
