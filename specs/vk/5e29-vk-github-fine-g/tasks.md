# Tasks: GitHub PAT Routing by Repository Owner

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` marks tasks in the same phase that touch
independent files and may be completed together.

## Phase 1: Routing and configuration core

- [x] T001 Add validated `githubAuth` options, normalized owner tables, the
  generated `gh` router package, and the runtime credential-preparation unit in
  `homelab/modules/vibe-kanban-rebuild.nix`.
- [x] T002 Wire the router PATH and credential prerequisite into coordinator and
  worker execution units without changing cluster payloads in
  `homelab/modules/vibe-kanban-rebuild.nix` (depends on T001).

## Phase 2: Validation and deployment documentation

- [x] T003 [P] Add Nix evaluation coverage for valid/invalid owner maps,
  runtime-only bootstrap paths, coordinator/worker unit dependencies, router
  PATH precedence, and default-disabled behavior in
  `homelab/tests/vibe-kanban-cluster.nix` (depends on T001–T002).
- [x] T004 [P] Add a derivation-backed fake-`gh` routing test covering explicit
  repo arguments, remote precedence and URL forms, owner case normalization,
  ambient-token override/fallback, and missing/empty configured credentials in
  `homelab/tests/vibe-kanban-github-auth.nix` (depends on T001–T002).
- [x] T005 [P] Document operator configuration, supported targets, security
  boundary, rotation, and troubleshooting in
  `homelab/docs/vibe-kanban-github-auth.md`, and link it from the module header in
  `homelab/modules/vibe-kanban-rebuild.nix` (depends on T001–T002).

## Phase 3: Host configuration and verification

- [x] T006 Configure the known per-owner 1Password references on execution
  nodes in `homelab/hosts/think/think2.nix`,
  `homelab/hosts/think/think3.nix`, and `homelab/hosts/think/think4.nix` when
  concrete references are available; concrete references were not supplied, so
  the feature remains safely disabled and the exact operator snippet is recorded
  in the deployment documentation (depends on T003–T005).
- [x] T007 Run Nix formatting, evaluate
  `homelab/tests/vibe-kanban-cluster.nix`, build/run
  `homelab/tests/vibe-kanban-github-auth.nix`, inspect rendered units, and scan
  the diff for secret/token leakage (depends on T003–T006).

## Phase 4: Required review and knowledge capture

- [x] T008 Run independent Codex review of both repository diffs, address all
  confirmed significant findings, and repeat focused verification until the
  review is clean (depends on T007).
- [x] T009 Distill reusable command-time credential-routing lessons into
  `vibe-kanban/docs/knowledge-base/workspace-environment-inheritance.md` (or a
  focused new topic), update `vibe-kanban/docs/knowledge-base/INDEX.md`, tag task
  `5e29-vk-github-fine-g`, and commit the knowledge-base update (depends on
  T008).
