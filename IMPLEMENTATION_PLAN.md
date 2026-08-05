# Implementation Plan: GitHub Fine-Grained PAT Routing

Task id: `5e29-vk-github-fine-g`

This initial plan follows `SPEC.md` and the constraints distilled in the
workspace-level `PRIOR_KNOWLEDGE.md`. SpecKit planning may refine file names and
task granularity after constitution/spec clarification.

1. **Inventory execution and deployment boundaries**
   - Trace the effective `PATH` and environment for local managed executions,
     setup/cleanup scripts, dev servers, workspace PTYs, cluster dispatch, and
     worker-side spawn.
   - Identify how `vibe-kanban-rebuild.nix` provisions runtime-only 1Password
     credentials and which identities run coordinator/worker processes.
   - Confirm the packaged absolute path of the real `gh` binary on each node.

2. **Define the non-secret routing contract**
   - Specify an owner-normalized manifest format mapping GitHub owner to a
     node-local credential path.
   - Add validation for owner syntax, case-insensitive duplicates, absolute
     runtime paths, and `/nix/store` rejection.
   - Define environment keys for the manifest and wrapper path as reserved,
     execution-owned configuration.

3. **Implement and test the `gh` router**
   - Build a small wrapper/helper that parses explicit `-R`/`--repo` targets,
     otherwise resolves the effective Git remote and parses GitHub.com SSH or
     HTTPS URLs.
   - Match configured owners case-insensitively, read only the selected token,
     set `GH_TOKEN` for the child, and exec real `gh` by absolute path.
   - Preserve caller authentication when no configured owner matches; emit
     secret-safe configuration errors for a matched owner with an invalid
     credential.
   - Test argument variants and precedence, remote selection/URL parsing,
     recursion prevention, fallback, empty/missing credentials, and token
     selection with a fake `gh` binary.

4. **Provision runtime credentials in Nix**
   - Add `githubOrgTokens` options to `vibe-kanban-rebuild.nix` using the
     established runtime credential and 1Password conventions.
   - Materialize owner-specific token files and a non-secret manifest with
     restrictive ownership/modes, without interpolating secrets into the Nix
     store, unit environment, or workspace.
   - Install/configure the router on coordinator and worker roles, and ensure
     rotation/restart behavior is deterministic.
   - Add Nix assertions or evaluation tests for invalid and disabled configs.

5. **Inject routing into every workspace process path**
   - Add the non-secret router configuration at the common local execution
     boundary before Vibe Kanban-owned workspace/session variables.
   - Pass the same configuration to workspace PTYs while leaving login and
     other non-workspace PTYs unchanged.
   - Ensure worker execution uses node-local paths and cluster action payloads
     contain no PAT values.
   - Add process-boundary tests for managed execution and PTY precedence.

6. **Document operator configuration and behavior**
   - Document owner matching, explicit `--repo` precedence, supported remote
     forms, fallback behavior, credential rotation, and troubleshooting.
   - Include a Nix example containing only 1Password/runtime references, never
     real token-shaped values.

7. **Verify in increasing scope**
   - Run focused wrapper/unit/process tests and Nix evaluation checks.
   - Install dependencies if required, format the Vibe Kanban repo, then run
     generated-type checks (if relevant), `pnpm run check`, lint, and targeted
     Rust tests.
   - Confirm with repository searches that no PAT content or token-shaped test
     secret entered tracked files and that dispatched payload types contain no
     credential field.

8. **Review and knowledge capture**
   - Run the required independent Codex diff review, fix confirmed significant
     findings, and repeat until clean.
   - Distill reusable command-time credential-routing and cluster deployment
     lessons into the project knowledge base, update its index, tag this task,
     and commit the knowledge-base update before handoff.
