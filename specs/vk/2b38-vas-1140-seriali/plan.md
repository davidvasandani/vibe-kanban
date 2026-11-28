# Implementation Plan: Serialized, time-bounded branch mirror to the source checkout

**Feature dir**: `specs/vk/2b38-vas-1140-seriali/` | **Spec**: `spec.md` | **Date**: 2026-11-28

## Summary

Replace the inline, synchronous, untimed `git push` in
`SharedRepositoryStore::mirror_branch_back` with a process-wide `SourceMirror`.
`SourceMirror` does the following:

- runs one leader task per (destination, branch), off the request path;
- skips no-op pushes;
- removes stale registrations that hold the branch, and fails deterministically
  on live ones;
- pushes with a timeout and a process-group kill;
- classifies failures and backs off;
- exposes the current failure to `RepoBranchStatus`.

Stale source-checkout registrations are also removed at three points: on adopt
(the moved worktree), on clustered workspace delete, and in a startup sweep.
Agent auto-commits that add very large amounts of content trigger a warning.

## Technical Context

**Language/Version**: Rust (edition per workspace) on tokio 1.50; TypeScript/React (web-core, ui)

**Primary Dependencies**: no new crates. Uses `tokio::process` (`process_group`), `nix` (via `utils`), `ts-rs`, and `serde`.

**Storage**: none. Failure state is in memory.

**Testing**: `cargo test -p utils -p workspace-manager -p git`; Vitest for `deriveRepoInfos`.

**Target Platform**: Linux coordinator (the NixOS host think2). Unix-only process-group handling is `cfg(unix)`-gated.

**Performance Goals**:
- the steady state spawns no push, only two `rev-parse` calls;
- triggers are O(1) and never await git.

**Constraints**:
- NFS-backed workspaces;
- no blanket `git worktree prune` (`crates/worktree-manager/src/worktree_manager.rs:893-907`).

**Scale/Scope**: about 300 workspaces and 120 stale registrations in one checkout.

## Constitution Check

| Principle | How this plan complies |
| --- | --- |
| II Test the contract | Each acceptance scenario maps to a Rust test (see tasks). |
| III Small, reversible steps | Reuses `GitCli` for `rev-parse` and the existing `RepoBranchStatus`/`RepoCard` surface. Adds no endpoint. `VK_SOURCE_MIRROR_DISABLED` switches it off. |
| XV Destructive ops fail safe | Registration removal needs positive evidence. Indeterminate means retain. The DB live-workspace guard applies at startup. Each removal is logged at `info!` before it happens. Work trees are never touched, only admin dirs. |
| XXI Failures say what failed | Messages name the repo, branch, destination, kind and git stderr. |
| XXVIII / XLIII Work outlives request; budgets | The mirror runs in a spawned leader. The request never waits. |
| LIX Hook-isolated | The push keeps `-c core.hooksPath=/dev/null`. |
| LXI (new) | Implemented directly. |
| Constraints | No new deps. Types are regenerated through `generate-types`. `pnpm run format` runs. |

Result: PASS, no violations.

## Project Structure

### Documentation (this feature)

```
specs/vk/2b38-vas-1140-seriali/
├── technical-spec.md  prior-knowledge.md  implementation-plan.md
├── spec.md  plan.md  research.md  data-model.md  tasks.md
└── contracts/repo-branch-status.md
```

### Source Code

```
crates/utils/src/worktree_linkage.rs              # + registrations(), registration_state(), remove_stale_registration()
crates/workspace-manager/src/source_mirror.rs     # NEW: SourceMirror, bounded git runner, classification, backoff
crates/workspace-manager/src/lib.rs               # re-exports
crates/workspace-manager/src/shared_repository.rs # mirror_branch_back → SourceMirror; adopt cleanup; forget_source_registrations
crates/local-deployment/src/container.rs          # delete-path cleanup; startup sweep; large-commit warning in commit_repos
crates/git/src/lib.rs (+cli.rs)                    # commit_added_size() helper
crates/server/src/routes/workspaces/git.rs        # RepoBranchStatus.source_mirror_error
crates/server/src/bin/generate_types.rs           # register SourceMirrorFailure(+Kind)
shared/types.ts                                   # regenerated
packages/web-core/src/pages/workspaces/gitPanelRepoInfo.ts (+ .test.ts)
packages/ui/src/components/GitPanel.tsx, RepoCard.tsx
packages/web-core/src/i18n/locales/*/tasks.json   # (path confirmed during implement)
```

## Phases

1. **Foundation**: the registration scanner in `utils`, with tests.
2. **SourceMirror**: the module, its tests, and wiring into `mirror_branch_back`.
3. **Registration cleanup**: adopt, delete and startup sweep.
4. **UI surface**: types, route, generated types, frontend.
5. **Large-commit guard**.
6. **Verify**: fmt, clippy, tests, check, lint.

## Complexity Tracking

None. The only new abstraction, `SourceMirror`, is required by FR-001 to FR-007.
