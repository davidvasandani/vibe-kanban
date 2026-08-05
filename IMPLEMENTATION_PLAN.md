# Implementation Plan: CLI Tools on Workspace Session PATH

1. Trace every workspace child-process boundary that can start an agent,
   setup/dev process, or interactive terminal locally or on a cluster worker.
2. Identify which boundaries already receive the app-managed CLI tools bin
   path and reproduce the missing clustered/session case with focused tests.
3. Extract or reuse a small environment helper that appends the canonical CLI
   tools bin directory after existing machine paths, preserves custom entries,
   deduplicates paths, and no-ops when the directory is unavailable.
4. Apply the helper at each execution-host spawn boundary that needs it. Derive
   the path on the actual execution host; change the coordinator/worker
   protocol only if the worker cannot derive the correct runtime location.
5. Add unit/integration coverage for host-first precedence, custom PATH
   preservation, deduplication, missing-directory behavior, and local/clustered
   workspace-session propagation.
6. Run focused Rust tests, formatting, the repository's relevant checks, and
   Nix parse/evaluation tests if `../homelab/modules/vibe-kanban-rebuild.nix`
   changes.
7. Cross-check the implementation against the feature spec and generated
   SpecKit tasks, then run an independent Codex diff review; fix and re-run
   verification until no significant findings remain.
8. Record any reusable execution-environment or clustered-path lesson in the
   project knowledge base, refresh its index, and commit the knowledge-base
   update separately before handoff.
