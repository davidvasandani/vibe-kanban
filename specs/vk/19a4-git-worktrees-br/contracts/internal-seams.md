# Contracts: internal seams

**No wire-protocol change.** `crates/cluster-protocol` is untouched:
`ExecutionDispatch` still carries `workspace_path` and `working_directory` and
nothing else path-shaped, and the request-signing contract is unchanged. The
contracts below are the internal seams this feature introduces.

---

## 1. `WorktreeLinkage` — the portability probe

**Location: `crates/utils/src/worktree_linkage.rs`.** Not `crates/git`:
`crates/worker` must call this and depends on `utils`, not on `git`, and putting
the probe in `crates/git` would drag `git2` into the worker. `crates/git`
re-exports nothing; the coordinator uses it from `utils` directly.

The contract has two halves with different dependency requirements, and they are
split deliberately:

- **`probe()` is pure filesystem** — no subprocess, no database, no `git2`. It
  reads `.git`, parses the `gitdir:` line, resolves `commondir`, and follows the
  store's `worktrees/<n>/gitdir` back. This is the half `crates/worker` runs, and
  the half that is cheap enough to run over every cluster workspace on every
  boot.
- **`assert_toplevel_is` is coordinator-only** and *may* spawn
  `git rev-parse --show-toplevel`. The worker never calls it.

The ancestor-repository hazard is still closed on the pure path: `git -C` walks
*up* to find a repository, but a `.git` **file** naming a resolvable
`worktrees/<n>` whose `gitdir` points back to *this exact worktree* cannot belong
to an ancestor — an ancestor's registration would name the ancestor's path. The
two-sided pointer check is what closes it; `assert_toplevel_is` is defence in
depth where a subprocess is affordable.

```rust
pub enum LinkageStatus {
    /// `.git` resolves, both pointers agree, common dir is inside the shared root.
    Portable { common_dir: PathBuf },
    /// Not a cluster worktree: a real `.git` directory, or a local placement.
    /// A distinct outcome from a failure — never rendered as "broken".
    NotApplicable { reason: &'static str },
    /// `.git` is a file whose `gitdir:` target does not exist.
    Dangling { target: PathBuf },
    /// Resolves, but to a common dir outside the shared root.
    OutsideSharedRoot { common_dir: PathBuf },
    /// A read or stat failed. Never collapses into `Portable`.
    Indeterminate { reason: String },
}

impl WorktreeLinkage {
    /// Pure filesystem. No subprocess. Callable from `crates/worker`.
    pub fn probe(worktree_path: &Path) -> LinkageStatus;
    /// Pure. Structural containment, not a substring test.
    pub fn assert_common_dir_within(&self, shared_root: &Path) -> Result<(), LinkageError>;
    /// Coordinator-only. May spawn `git rev-parse --show-toplevel`.
    pub fn assert_toplevel_is(&self, expected: &Path) -> Result<(), LinkageError>;
}
```

**Guarantees**

- Uses `try_exists()`, never `Path::exists()`; never
  `read_dir(..).filter_map(|e| e.ok())`. On this storage a stat failure is
  routine, and both idioms turn *indeterminate* into *clean*.
- Verifies **both** directions: `{worktree}/.git` → `{store}/worktrees/<n>`, and
  `{store}/worktrees/<n>/gitdir` → `{worktree}/.git`. This is also what closes
  the ancestor-repository hazard without a subprocess.
- `assert_common_dir_within` is a structural containment check. It must never be
  implemented as "the path does not contain `/srv/src`".
- `probe` and `assert_common_dir_within` add no dependency beyond `std`/`tokio`
  filesystem access, so `crates/worker` gains nothing from calling them.
- Every probe is bounded by a timeout.

---

## 2. `SharedRepositoryStore` — the coordinator-owned backing repository

```rust
impl SharedRepositoryStore {
    /// Takes the lock manager: every operation below is fenced by the
    /// repository administration lease, which is not derivable from a root path.
    pub fn new(
        shared_root: impl Into<PathBuf>,
        locks: Arc<RepositoryAdminLockManager>,
    ) -> Result<Self, WorkspaceError>;

    pub fn path_for(&self, repo_id: Uuid) -> PathBuf;

    /// Idempotent. Safe to call on every provisioning.
    pub async fn ensure(&self, repo: &Repo, target_branch: &str)
        -> Result<PathBuf, WorkspaceError>;

    /// Re-links an existing worktree in place. Never recreates it.
    /// Takes `&Repo`: the store is *not* derivable from the worktree, whose
    /// pointer names `/srv/src/<repo>` and identifies no store, and adoption
    /// also needs the repo to read the old tip from and to push back to.
    pub async fn adopt(&self, repo: &Repo, worktree_path: &Path, branch: &str)
        -> Result<AdoptOutcome, WorkspaceError>;
}

pub enum AdoptOutcome {
    AlreadyPortable,
    Adopted { common_dir: PathBuf },
    Skipped { reason: SkipReason },
}
```

### Precondition: the administration lease does not nest

`ensure`, `adopt` and `WorkspaceManager::create_workspace_fenced` each acquire
the repository administration lease around their **own** critical section and
**fully release it before returning**. None of the three is ever called while
another holds it, for the same repository or otherwise.

This is load-bearing, not stylistic. `RepositoryAdminLockManager::acquire`
(`crates/worktree-manager/src/worktree_manager.rs:117-151`; `:107` is `pub fn new`)
takes an owned `tokio::sync::Mutex` guard and *then* an exclusive SQLite lease.
A nested acquire for one repository deadlocks on the in-process mutex — it never
returns to release the outer guard — and a genuinely concurrent one returns
`RepositoryLockBusy`. The provisioning order is therefore
`ensure` → (released) → `create_workspace_fenced`, and lazy repair is
`ensure` → (released) → `adopt` → (released). A regression test provisions one
repository from two tasks concurrently and asserts one store, no deadlock, and no
spurious `RepositoryLockBusy`.

### `ensure` contract

| # | Guarantee |
| --- | --- |
| E1 | Idempotent; early-outs when the store exists and resolves `target_branch`. |
| E2 | The clone runs **outside** the repository administration lease, into a per-attempt `.{repo_id}.incoming`. The lease covers only verify → configure → `rename(2)` → fetch, which a bounded SQLite lease can span. Constitution XII: no coordination lock is held across an awaited external operation. |
| E2a | Concurrent `ensure` calls for one repository are deduplicated **by the lease**, by re-running the E1 early-out as the first step under it: the loser observes the winner's published store and discards its staging directory. No in-process mutex is used or minted — `repository_operation_lock` is private to `crates/worktree-manager` (`worktree_manager.rs:189`) and unreachable from `crates/workspace-manager`. |
| E3 | Publication is a `rename(2)`. A partially created store is never observable as valid. |
| E4 | gc configuration is written **before** the first worktree is added. Ordering is load-bearing: `git gc --auto` fires on ordinary commands, prunes worktrees, and could otherwise drop another workspace's registration from a *worker*. |
| E5 | The directory is setgid, group-owned, umask `002`, before the first object is written. |
| E6 | Remotes are copied from `repo.path` and `origin` is retargeted to the real forge. `git clone --bare` would leave `origin` naming the local checkout, so `origin/main` would mean the wrong thing. |
| E7 | Returns `Err` unless `target_branch` resolves in the store afterwards, proven with `git cat-file -e <sha>^{commit}`. A created directory is not evidence. |
| E8 | Best-effort push of the workspace branch back into `repo.path` for rollback safety; its failure is advisory and does not fail `ensure`. |

### `adopt` contract

| # | Guarantee |
| --- | --- |
| A1 | Mutates **no** file in the working tree. Pointer files and the index only. |
| A2 | Every write is same-directory temp + `rename(2)`. The `.git` marker is never transiently unlinked — a directory with no `.git` in any subdirectory is classified as holding no work and becomes deletable by the orphan sweep. |
| A3 | Refuses (does not guess) when `.git` cannot be parsed, when it is a real directory, when the branch cannot be proven present, or when the branch is already checked out by another worktree of the store. |
| A4 | Never falls back to the target branch when the workspace branch is missing — that would silently discard commits. |
| A5 | Fails **before** any filesystem mutation when it cannot complete. No half-migrated worktree. |
| A6 | Idempotent; `AlreadyPortable` is a cheap no-op. |
| A7 | Re-probes after `git worktree repair`. A zero exit is not verification. |
| A8 | Logs the path, why it was selected, and the action at `info!` **before** acting. |
| A9 | Makes no database writes. |

---

## 3. `workspace_repo_git_path` — the one resolver

```rust
async fn workspace_repo_git_path(&self, workspace: &Workspace, repo: &Repo)
    -> Result<PathBuf, ContainerError>;
```

| # | Guarantee |
| --- | --- |
| R1 | Returns `repo.path` when clustering is disabled, and when `placement_state == local`. This is exactly PR #172's distinction and must not be re-broken. |
| R2 | Returns the store for `reserved`/`provisioning`/`ready`/`failed`/**`cleaning`**. `WorkspacePlacementState` has six variants (`crates/db/src/models/workspace.rs:105-113`); `cleaning` belongs here because cleanup of a cluster workspace must administer its worktrees in the store they were created from, and falling through to `repo.path` there is the FR-24 failure. |
| R2a | The match is **exhaustive over the enum, with no wildcard arm**, so a seventh variant is a compile error rather than a silent fallthrough to `repo.path`. |
| R3 | Reads the **persisted** placement row. Affinity is never inferred from the host serving the request. |
| R4 | There is exactly one such resolver. No route re-implements the branch. |
| R5 | Conveys nothing through the process environment: no global `GIT_DIR`/`GIT_COMMON_DIR`, no mutated global Git config. Per-workspace state must not leak across concurrent workspaces. |
| R6 | Applied at **every** child-process boundary — the container service and the PTY/terminal path. Fixing only the executor path leaves interactive terminals resolving the wrong store. |

**Deliberately not resolved** (`crates/server/src/routes/repo.rs:92,105,167,223,260,263`,
and `crates/local-deployment/src/container.rs:2060`): "open in editor",
repository search, repo-level branch/remote listing, and `copy_project_files`.
These describe the registered repository, and a bare store has no working tree —
`copy_project_files` in particular copies *out of* that working tree, so routing
it at the store would break `copy_files` for every cluster workspace. A comment
at each site records the decision.

---

## 4. Worker preflight

An extension of the existing dispatch admission path
(`crates/worker/src/execution.rs`, after `authorize_workspace_path`), not a new
endpoint — so it inherits the request-signing contract unchanged.

| # | Guarantee |
| --- | --- |
| W1 | Probes each repo directory found by `discover_repo_names` using contract 1's `probe()` and `assert_common_dir_within` — the pure-filesystem half, from `crates/utils`, which `crates/worker` already depends on. It never calls `assert_toplevel_is`, which may spawn `git`. Filesystem reads only; **no** `git`/`git2`, `db`, `worktree-manager` or `workspace-manager` dependency is added to `crates/worker`. The ancestor-repository hazard is covered anyway by the two-sided pointer check. |
| W2 | Rejects the dispatch with a typed reason when linkage does not resolve, and the worker job reaches a **terminal** state — a failed dispatch left pending contaminates reconciliation. |
| W3 | Never repairs. Administration authority stays with the coordinator. |
| W4 | Never satisfies a dangling pointer with a same-named local directory. Existence proves nothing. |
| W5 | Distinguishes *not applicable* from *broken*; the former is not an error. |
