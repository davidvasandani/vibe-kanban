# Tasks: Shared cluster store keeps a local base branch fresh before serving it

**Plan**: `./plan.md`

## Phase 1: Primitive

- [x] T001 Add `GitCli::is_ancestor` in `crates/git/src/cli.rs` (next to
      `merge_base`): wraps `git merge-base --is-ancestor`, non-zero exit
      reads as `Ok(false)` per the existing `commit_exists` convention.

## Phase 2: Core (depends on T001)

- [x] T002 Add `SharedRepositoryStore::fast_forward_local_head` in
      `crates/workspace-manager/src/shared_repository.rs`: resolve the
      store's `origin` URL, fetch the single branch into
      `refs/remotes/origin/<branch>`, compare tips with `is_ancestor`,
      fast-forward via `update-ref` when behind, return
      `WorkspaceError::SharedStore` when diverged.
- [x] T003 Wire the call into `SharedRepositoryStore::ensure`'s cheap path
      (same file), immediately after `store_resolves` returns `true`.
- [x] T004 Update the stale doc comment on `store_resolves` that described
      local-head staleness as deliberately left alone (same file).

## Phase 3: Validation

- [x] T005 [P] Add `ensure_fast_forwards_a_stale_local_target_branch_to_origin`
      in `crates/workspace-manager/src/shared_repository.rs` (`mod tests`).
- [x] T006 [P] Add `ensure_refuses_a_diverged_local_target_branch` in the
      same test module.
- [x] T007 [P] Confirm by inspection that no pre-existing test in the file
      calls `ensure` with a bare local branch name against a configured
      `origin` remote, so none are affected by this change (recorded in
      `implementation-plan.md`; no file changed).
- [x] T008 `cargo test -p workspace-manager -p git --lib` passes, including
      the two new tests (33/33, including both new tests).
- [x] T009 `cargo fmt` and `cargo clippy` clean for the touched crates.

## Phase 4: Governance & ship

- [x] T010 Extend constitution Principle LIX in
      `.specify/memory/constitution.md` with the shared-store freshness
      rule; bump version.
- [x] T011 Independent review pass (Codex CLI) over the diff; address
      confirmed findings:
      - Split `ensure` into `ensure` (freshness-checked) and a new
        `ensure_for_heal` (never freshness-checked) in
        `crates/workspace-manager/src/shared_repository.rs`; moved
        `heal_cluster_worktree`'s call in
        `crates/local-deployment/src/container.rs` to the latter — it
        materialises a workspace's own branch ahead of adoption, not a base
        branch.
      - Added `SharedRepositoryStore::branch_is_checked_out` guard (same
        file) as defense-in-depth.
      - Added `ensure_never_moves_a_checked_out_branchs_ref` test (same
        file, `mod tests`).
      - Made the fast-forward `update-ref` a 3-arg CAS with a bounded
        single retry on contention (same file).
      - Unified the initial freshness check and the CAS retry's re-check
        behind one `classify_against_remote` helper (equal/behind/ahead/
        diverged) after two further review passes found both checks could
        misreport an already-fresher-than-fetched local branch as
        diverged. Added `classify_against_remote_distinguishes_ahead_from_diverged`.
      - Final review pass confirmed no remaining significant findings
        beyond the already-accepted `is_ancestor` convention.
- [ ] T012 Distill reusable knowledge into `docs/knowledge-base/` (update
      `clustered-workspace-execution.md` and the index) if anything new
      survives review.
- [ ] T013 Open and merge the PR against `main`.
