# Tasks — `b72a-internal-error-o`

Plan: [`plan.md`](plan.md) · Seams: [`contracts/internal-seams.md`](contracts/internal-seams.md)

`[P]` = touches files no other task in the same layer touches, so it can run
concurrently with its siblings.

## Layer 0 — baseline

- [x] **T001** Confirm `cargo build --workspace --tests` passes on unmodified
      `main` (`293f7017`). Nothing to change; this is the control.

## Layer 1 — independent leaves

- [x] **T002** `[P]` Add `GitCli::fetch_with_refspecs(repo_path, remote_url, &[&str])`
      and reimplement `fetch_with_refspec` as a one-element delegation, keeping
      `GIT_TERMINAL_PROMPT=0` and `classify_cli_error`. Seam S1.
      → `crates/git/src/cli.rs`
- [x] **T003** `[P]` Add `WorkspaceError::SharedStore { repo_name, branch, detail }`.
      Seam S5.
      → `crates/workspace-manager/src/workspace_manager.rs`
- [x] **T004** `[P]` Add `ContainerError::SharedStore(String)` with `#[error("{0}")]`.
      Seam S5.
      → `crates/services/src/services/container.rs`

## Layer 2 — the store's own logic (depends on T002, T003)

All four touch `crates/workspace-manager/src/shared_repository.rs`, so they are
sequential with respect to each other.

- [x] **T005** Add `resolved_branch_ref` (local ref, then remote ref, presence
      proven with `commit_exists`) and reimplement `branch_commit_present` on
      top of it, keeping its signature. Doc comment names
      `GitService::find_branch` as the rule being matched. Seam S2. *(needs T003
      only for the later steps; independent of T002.)*
      **Must not modify `commit_presence_is_proven_not_assumed`** — that existing
      test is what proves AC 4 (analysis W1).
- [x] **T006** In `publish_and_fetch`, mirror
      `+refs/heads/*:refs/heads/*` **and** `+refs/remotes/*:refs/remotes/*` from
      the registered checkout in one best-effort `fetch_with_refspecs`, still
      inside the lease and after `configure()` + rename. Additive; no `--prune`.
      Seam S4. *(needs T002.)*
- [x] **T007** Extract `fallback_refspec(remote_name, target_branch)`; use it in
      the remotes loop and break on the branch being *present* afterwards rather
      than on a zero exit. Seam S3.
- [x] **T008** Return `WorkspaceError::SharedStore { .. }` from `ensure`'s closing
      "does not resolve target branch" check, naming repository and branch; log
      the resolved ref form at `info!` before provisioning proceeds. Seam S5.
      *(needs T003, T005.)*

## Layer 3 — error plumbing (depends on T003, T004, T008)

- [x] **T009** `[P]` Map `WorkspaceError::SharedStore` → `ContainerError::SharedStore`
      in `map_workspace_manager_error`.
      → `crates/local-deployment/src/container.rs`
- [x] **T010** `[P]` Add `ApiError::ClusterProvisioning(String)`; add the
      `ContainerError::SharedStore` arm to `From<ContainerError> for ApiError`
      **before** the `other =>` catch-all; render as
      `with_status(INTERNAL_SERVER_ERROR, "ClusterProvisioningError", msg)`.
      → `crates/server/src/error.rs`

## Layer 4 — tests (depends on Layer 2)

All in `crates/workspace-manager/src/shared_repository.rs`'s `mod tests`, so
sequential with respect to each other.

- [x] **T011** Add the `store_with_locks` helper (in-memory `sqlite::memory:`
      pool, `max_connections(1)`, hand-written `CREATE TABLE
      repository_admin_locks`, per `crates/worktree-manager/src/worktree_manager.rs:45-66`)
      and a `seed_checkout_with_remote` fixture that gives a repo a real
      `refs/remotes/origin/main`.
- [x] **T012** `ensure_serves_a_remote_prefixed_target_branch` — AC 1, AC 2:
      `ensure(repo, "origin/main")` succeeds; the store resolves `origin/main`;
      `GitService::create_branch` and `worktree_add` from it succeed.
- [x] **T012a** Extend T012: seed a ref in the store that the registered checkout
      does not have (a `vk/…` head standing in for another workspace) and assert
      it **survives** `ensure`. Proves FR-5, which T006's "no `--prune`" only
      asserts as an instruction (analysis E1).
- [x] **T013** `branch_resolution_prefers_a_local_branch_over_a_remote_one` — AC 3.
- [x] **T013a** `resolved_branch_ref_agrees_with_git_services_branch_lookup` —
      assert `resolved_branch_ref(..).is_some()` equals
      `GitService::check_branch_exists(..)` across the truth table, pinning the
      duplicated local-then-remote rule to the one it copies (analysis W2,
      constitution XXI).
- [x] **T014** `fallback_refspec_targets_the_remote_tracking_namespace` — AC 5,
      truth table over the four rows in seam S3.
- [x] **T015** `ensure_reports_which_repository_and_branch_it_could_not_serve` —
      AC 6: the closing check still fires, now as `WorkspaceError::SharedStore`.
- [x] **T015a** `[P]` In `crates/server/src/error.rs`, assert
      `ApiError::from(ContainerError::SharedStore(msg))` renders 500 with `msg`
      verbatim and **not** "An internal error occurred". Pins the two match arms
      (T009, T010) that are where the message is lost today, and the arm ordering
      against the `other =>` catch-all (analysis E2).
      → `crates/server/src/error.rs`

## Layer 5 — verification (depends on everything)

- [x] **T016** Confirm T012–T015 **fail** against unmodified `main`. A test that
      also passes before the fix proves nothing.
- [x] **T017** `cargo test --workspace`.
- [x] **T018** `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] **T019** `pnpm run format`; run `pnpm run check` if pnpm is usable on this
      node, and record it in the PR if it is not.
- [x] **T020** Read-only re-check of the live shared store from think3, and of
      the reasoning against `/srv/src/homelab`'s real refs. No dev server.

## Not in this list

The three deferred defects in `spec.md`'s Out of Scope (request timeout, worker
endpoint negative caching, workspace rollback on provisioning failure). Each is
real and on the same request path; none causes this report.

## Execution notes

- **T002 needed a second call site the plan missed.** `WorkspaceError` is matched
  exhaustively in *two* places, not one: `map_workspace_manager_error`
  (`crates/local-deployment/src/container.rs`, the container path) and
  `From<WorkspaceManagerError> for ApiError` (`crates/server/src/error.rs`, the
  handler path used by `add_repository`). The compiler caught the second; both
  now route `SharedStore` to `ApiError::ClusterProvisioning`.
- **T016 control.** Reverting only the two behaviours — resolution back to
  `refs/heads` only, and the mirror back to heads only — makes
  `ensure_serves_a_remote_prefixed_target_branch`,
  `branch_resolution_agrees_with_git_services_branch_lookup` and
  `ensure_never_removes_refs_another_workspace_may_hold` fail, and leaves the
  other 22 green. `branch_resolution_prefers_a_local_branch_over_a_remote_one`
  passes either way by design: it pins ordering, not the new capability.
- **T017/T018.** `cargo test --workspace --exclude vibe-kanban-tauri`: 70 suites,
  0 failures. `cargo clippy --workspace --exclude vibe-kanban-tauri --all-targets
  -- -D warnings`: clean. `vibe-kanban-tauri` is excluded because webkit2gtk is
  not in this node's `/nix/store`; CI excludes it too.
- **T019.** `cargo fmt --all` applied (workspace and `crates/remote`). **pnpm is
  not installed on this worker**, so `pnpm run check` / `pnpm run lint` could not
  run. The change is backend-only and `shared/types.ts` is unchanged — `ApiError`
  is `#[ts(type = "string")]`, so a new variant does not alter the generated
  type. Recorded in the PR, as #174 did for the same reason.
- **T020.** Both live stores currently lack `refs/remotes/origin/main`, which is
  the defect; `/srv/src/homelab` has it, so the first `ensure` after this deploys
  backfills the homelab store. (`/srv/src/vibe-kanban` could not be checked from
  this node — it lives on the coordinator.) No migration is needed.
- A `PRIOR_KNOWLEDGE.md` claim was corrected during execution: the orphan-sweep
  hazard quoted from `workspace-directory-reclamation.md` is inert while
  clustering is enabled (`container.rs:1153` → `workspace_manager.rs:778`). The
  "no dev server on this host" rule stands, for contention rather than
  reclamation.

## Review round (stage 11)

- [x] **T021** `crates/git/src/cli.rs`: revert `fetch_with_refspecs`; document why
      one refspec per invocation is load-bearing (analysis R1).
- [x] **T022** `shared_repository.rs`: issue the two mirrors separately with
      independent error handling (R1).
- [x] **T023** Add `local_branch_commit_present`; use it in `store_resolves` (R2),
      `adopt` (R3) and `mirror_branch_back` (R4).
- [x] **T024** Restrict the fallback loop to the remote whose name prefixes the
      target branch (R5).
- [x] **T025** Log and surface fallback fetch failures instead of discarding them
      (R6).
- [x] **T026** `a_tag_is_not_a_branch` — pins the rejection of git's bare-name
      precedence, which also accepts tags.
- [x] **T027** `the_remote_tracking_mirror_survives_a_checked_out_branch` (R1) and
      `a_moved_target_branch_is_picked_up_by_the_next_provisioning` (R2).
- [x] **T028** `adopt_refuses_a_branch_that_is_only_a_remote_tracking_ref` (R3).
- [x] **T029** Control: both new regression tests fail against the flawed first
      version (batched refspec restored, `store_resolves` widened) and pass after.
- [x] **T030** Re-verify: 70 suites green, clippy clean, `cargo fmt --all`.
