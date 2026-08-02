# Prior knowledge — `19a4-git-worktrees-br` (broken Git worktrees in cluster mode)

Distilled from the four knowledge bases this workspace can reach:

- `vibe-kanban/docs/knowledge-base/` (20 content pages plus `INDEX.md`) — the
  primary VK knowledge base
- `vibe-kanban/wiki/` (19 pages)
- `homelab/docs/knowledge-base/` (27 pages) and `homelab/knowledge-base/` (12 pages)

Every constraint below is load-bearing for this task. Constraints marked **[H]**
are hazards that change the design rather than merely constrain it.

---

## A. Authority and ownership (who may touch Git)

1. **Only the coordinator may add, remove, prune or reclaim worktrees, or delete
   shared branches. Workers run ordinary Git commands inside their assigned
   worktree only.** (`clustered-workspace-execution.md`)
   → Store creation, adoption and repair are coordinator-only code paths. The
   worker may *detect* and *refuse*, never repair.

2. **[H] "Coordinator writes, workers read" is not achievable and must not be
   claimed.** A linked worktree's `index`, `HEAD`, `ORIG_HEAD` and `logs/` live
   inside the *store's* `worktrees/<n>/` directory, so every worker Git command —
   even `git status` — writes into the shared store.
   → The design must be "single-writer *administration*, many ordinary writers",
   and the ownership/permission model must assume concurrent multi-node writes.

3. **Serialize per-repository administration with *fenced* ownership; a plain
   lock file cannot distinguish a live owner from a stale one.**
   (`clustered-workspace-execution.md`)
   → `RepositoryAdminLockManager` (SQLite generation lease) wraps store
   provisioning and adoption, not just `worktree add`.

4. **Persist affinity; never infer it from the UI host.**
   (`clustered-workspace-execution.md`)
   → The git-path resolver decides "cluster-placed" from the persisted
   `workspace_placement` row, never from the request.

5. **Verify the effective service account before designing ownership.** think2
   runs the agent launcher as `vibe-kanban-dev` while a separate `vibe-kanban`
   account is retained "for cluster maintenance".
   (`homelab/docs/knowledge-base/vk-app-managed-cli-tools.md`)
   → Confirm the storage-side mapped identity (`VK_WORKER_EXPECTED_UID=977`,
   `GID=988`) is what both principals actually produce on the export.

## B. Shared-storage mechanics

6. **A shared mount is a capability, not a directory: mount identity, coordinator
   probe, writability, mapped UID/GID and capacity are all checked before a
   worker is schedulable.** (`clustered-workspace-execution.md`)
   → Worker preflight for the store extends this existing check; it does not add
   a parallel `path.exists()` test.

7. **"An existing path does not prove that NFS is mounted… do not fall back to an
   identically named local directory."** (`clustered-workspace-execution.md`)
   → This is literally the observed bug: `/srv/src/homelab` exists on the worker
   but is a *different* repository. Preflight must never accept it.

8. **[H] Shared writable state fails on lock *inodes*, not directory
   permissions.** A `0644` lock file created by one principal could not be
   reopened by another despite correct group and directory write access. Fix:
   setgid group-owned directories plus a `002` umask, applied *before* the first
   write. (`homelab/docs/knowledge-base/vk-app-managed-cli-tools.md`)
   → The store is full of lock files (`config.lock`, `packed-refs.lock`,
   `refs/**/*.lock`, per-worktree `index.lock`). Set setgid + umask at creation.

9. **Atomic materialisation on shared storage: per-resource staging directory →
   verify → `rename(2)` → expose. A *shared* staging root breaks per-resource
   locking.** (`vk-app-managed-cli-tools.md`)
   → Clone into `repositories/.{repo_id}.incoming`, then rename. Never a shared
   `tmp/` staging directory.

10. **Atomic file replacement: temp file in the *same* directory, write + fsync,
    rename over the target; refuse to write when the existing file cannot be
    parsed; serialize read-modify-write.** (`aws-sso-profile-management.md`)
    → Applies to rewriting each worktree's `.git` pointer and the store's
    `worktrees/<n>/gitdir`.

11. **Use `try_exists()`, never `Path::exists()` (which returns false for both
    "absent" and "stat failed"), and never
    `read_dir(..).filter_map(|e| e.ok())`.**
    (`workspace-directory-reclamation.md`)
    → NFS stat failures are routine here; both traps turn "indeterminate" into
    "clean" and would make a sweep delete live work.

12. **Disk capacity for `{shared_root}` is undocumented — treat as unverified.**
    (absent from both KBs)
    → Measured during this task: 35 TB free of 37 TB. One store per repo (not per
    workspace) keeps growth bounded. `objects/info/alternates` is rejected: it
    re-introduces an absolute path, i.e. the exact bug being fixed.

## C. Worktree registration and pruning

13. **[H] Never run a repo-wide `git worktree prune`.** On 2026-07-05 it walked
    every registration in `/srv/src/vibe-kanban` and died with `Permission
    denied` on the app's foreign-owned registrations, killing the build.
    (`wiki/self-hosted-deployment.md`)

14. **[H] Known-unfixed: `comprehensive_worktree_cleanup` ends with a repo-wide
    `git worktree prune`, so one workspace's cleanup already drops other live
    workspaces' admin entries.** (`workspace-directory-reclamation.md`)
    → Consolidating every cluster workspace of a repo onto **one** shared store
    raises this from per-node to cluster-wide. Scoping or removing that prune is
    now in scope for this task, not a follow-up.

15. **Scope cleanup to the worktree you created: `worktree remove --force` plus
    removal of *your own* `.git/worktrees/<name>` admin directory.**
    (`wiki/self-hosted-deployment.md`)

16. **[H] Admin directory names are derived by Git from the path *basename*, not
    the workspace id — verified safe under the current layout, not assumed.**
    (`workspace-directory-reclamation.md`)
    → That verification does **not** carry over. Every cluster worktree of a repo
    is named `<repo_name>`, so one store's `worktrees/` namespace fills with
    `<repo>`, `<repo>1`, `<repo>2`… — one entry per live workspace instead of a
    handful per node.
    → VK's own cleanup is **not** basename-derived and must not be rewritten as
    if it were: `force_cleanup_worktree_metadata`
    (`crates/worktree-manager/src/worktree_manager.rs:755-777`; `:718-748` is the
    retry block) resolves through `find_worktree_git_internal_name` (`:573-610`),
    which reads every `worktrees/*/gitdir` and compares canonicalised paths. The
    real defect at that site is error-swallowing, and constraint 11 names it:
    `read_dir(...).filter_map(|entry| entry.ok())` (`:583-585`) and
    `gitdir_path.exists()` (`:599`) make a transient NFS read failure return
    `Ok(None)`, after which the caller falls through to a broader cleanup against
    a namespace that now holds every workspace of the repo.

17. **A registration outliving its directory is a known live failure mode.**
    (`wiki/agent-process-lifecycle.md`)
    → Repair reconciles both directions: dangling `.git` pointers *and* stale
    admin entries with no worktree.

18. **[H] The pointer is two-sided.** `{worktree}/.git` → `{store}/worktrees/<n>`,
    and `{store}/worktrees/<n>/gitdir` → `{worktree}/.git`. Repairing only one
    leaves a dangling registration — the precondition of the prune incident above.

19. **[H] `git -C <path>` walks *up* to find `.git`; a "repaired" worktree can
    silently resolve to the wrong repository.**
    (`homelab/docs/knowledge-base/family-os-git-hash-deploy-visibility.md`)
    → The coordinator asserts `rev-parse --show-toplevel` equals the worktree
    path **and** `--git-common-dir` resolves under
    `{shared_root}/repositories/{id}`. The worker cannot: it runs the
    pure-filesystem probe with no subprocess. The hazard is still closed there by
    the two-sided pointer check (18) — a `.git` *file* naming a resolvable
    `worktrees/<n>` whose `gitdir` points back to this exact worktree cannot
    belong to an ancestor, because an ancestor's registration would name the
    ancestor's path.

20. **`git rev-parse` echoes any well-formed 40-hex string whether or not the
    object exists; prove presence with `git cat-file -e <sha>^{commit}`.**
    (`homelab/knowledge-base/pinning-upstream-revisions.md`)
    → Prove the store holds each worktree's HEAD *before* repointing it, or every
    broken worktree gets repointed at a store that lacks its objects.

21. **Two cluster worktrees cannot hold the same branch; repair must not steal a
    branch from a live workspace.**
    (`homelab/docs/knowledge-base/deployment-checkout-recovery.md`)
    → Branch-checkout exclusivity is now fleet-wide, because all worktrees of a
    repo share one ref store.

## D. Destruction, preservation, and the sweeps

22. **Retain on dirty *and* on indeterminate. "A Git probe that errors is not a
    clean repo."** (`workspace-directory-reclamation.md`)
    → A dangling gitdir makes *every* probe error, which is exactly the
    indeterminate case. All 15 broken worktrees (9 cluster workspaces, 100% of
    the cluster-placed fleet) must be retained.

23. **[H] Sequence repair before capture.** The recovery playbook says capture
    branch/HEAD/status/staged+unstaged/reflog *before* mutating — but `git status`
    currently fails in every broken worktree.
    (`deployment-checkout-recovery.md`)
    → Order must be: rewrite pointers only (non-destructive) → capture state →
    only then consider anything that could lose work. Never re-clone-and-replace
    a worktree directory.

24. **Destructive steps log path, reason and action at `info!` *before* acting;
    "a cleanup that returns `Ok(())` after a failed removal is worse than
    useless."** (`workspace-directory-reclamation.md`, constitution XV)

25. **Never transiently remove a worktree's `.git` marker.** A directory with no
    `.git` in any subdirectory is classified as holding no work and becomes
    deletable. (`workspace-directory-reclamation.md`)
    → Pointer rewrites must be same-directory temp + rename, never unlink-then-write.

26. **Orphan classification is an un-canonicalised exact string compare against
    `workspaces.container_ref`.** (`workspace-directory-reclamation.md`)
    → Repair must not rewrite `container_ref`, and must use exactly one canonical
    path form everywhere (see 33).

27. **There are two independent sweeps** — `cleanup_expired_workspaces`
    (DB-aware, 30 min) and `cleanup_orphan_workspaces` (filesystem-only, once per
    boot) — **and fixing one does not fix the other.**
    (`workspace-directory-reclamation.md`)

28. **Do not run a dev server on this host to test the sweeps.**
    `cleanup_orphan_workspaces` always sweeps the default base dir even with an
    override, and against a non-matching DB it would classify every live worktree
    there as an orphan. (`workspace-directory-reclamation.md`)
    → **The sweep is nevertheless inert in this fleet**:
    `crates/workspace-manager/src/workspace_manager.rs:714-719` early-returns
    when `allow_reclamation == false`, and
    `crates/local-deployment/src/container.rs:1025` passes
    `!cluster_config.enabled`. Clustering is on everywhere here, so it never
    runs — the "runaway orphan sweep" is not the hazard it was written up as.
    The prohibition stands for a different reason: a second server on a cluster
    host contends for the same shared root, the same SQLite repository
    administration leases, and the same live worktrees this task is repairing.

29. **Archived workspaces enter a 1 h cleanup window instead of 72 h.**
    (`issue-status-side-effects.md`)
    → Triage ordering matters; the cleanup queries must know about the store.

30. **Preservation is never conditional on an unrelated step succeeding, and
    multi-repo capture is best-effort per repo with a truthful aggregate.**
    (`interrupted-worktree-recovery.md`, constitution XV)
    → Batch adoption reports per-worktree outcomes; a failure on one repo of a
    multi-repo workspace does not abort the others or claim success.

31. **A failing operation must fail *before* any filesystem mutation, not
    half-way.** (`interrupted-worktree-recovery.md`)
    → No half-migrated worktree with a rewritten pointer but absent objects.

32. **The `Running` row left by a failed stop is load-bearing; do not
    opportunistically terminalise it.** (`interrupted-worktree-recovery.md`)

## E. Invariants, resolvers and convergence

33. **Canonicalise once at the boundary and keep one representation.**
    (`remote-external-integrations.md`)
    → Two path forms make the level-triggered portability check flap and can
    misclassify a live workspace as an orphan (26).

34. **Resolve once behind a single workspace-scoped resolver; consumers receive
    the resolved value explicitly.** (`workspace-environment-inheritance.md`)
    → Exactly one workspace→git-path resolver. No duplicated cluster branching in
    routes.

35. **A workspace has multiple child-process boundaries — `ContainerService`
    (`crates/local-deployment/src/container.rs`) and `PtyService`
    (`crates/local-deployment/src/pty.rs`); fixing one leaves the other with the
    wrong environment.** (`workspace-environment-inheritance.md`)

36. **Never convey per-workspace configuration through the long-lived server
    environment or a written config file.**
    (`workspace-environment-inheritance.md`)
    → No process-global `GIT_DIR`/`GIT_COMMON_DIR`, no mutated global Git config.

37. **Edge triggers stall silently; only a periodic level-triggered reconciler
    converges, and existence checks are not enough.**
    (`wiki/self-hosted-deployment.md`, `issue-status-side-effects.md`)
    → The portability invariant is enforced on boot, on placement and on
    dispatch, with per-workspace failure isolation — not once at creation.

38. **"A digest that nothing re-checks on a schedule is a comment, not a
    control."** (`forked-mcp-server-packaging.md`)
    → A one-off migration without a recurring check regresses the moment an
    unfixed path creates a worktree.

39. **Guard on actual change and early-out when nothing differs.**
    (`issue-status-side-effects.md`)
    → Adoption must be a cheap no-op for already-portable worktrees so it can run
    on every boot over the whole cluster-placed fleet.

40. **Prerequisites are validated before the first mutating stage; gather *all*
    missing items and emit the exact recovery command.**
    (`worktree-formatting-prerequisites.md`)

41. **Do not silently auto-remediate inside a routine command.**
    (`worktree-formatting-prerequisites.md`)
    → Repair is explicit, coordinator-owned and predictable; it is not a side
    effect of an ordinary Git call.

42. **Classify "not applicable" as its own status, never as a false failure, and
    bound every probe with a timeout.** (`mcp-connectivity-testing.md`)
    → NFS calls hang; a local-placed workspace is `NotApplicable`, not `Broken`.

43. **A zero exit is not verification — confirm with an independent probe.**
    (`cli-tool-oauth-login.md`)
    → `git worktree repair` returning 0 does not prove portability; re-probe.

44. **Encode the invariant as a parsed-structure assertion, not a string match.**
    (`forked-mcp-server-packaging.md`)
    → Assert the resolved common dir is *under `{shared_root}`*, not that it does
    not contain `/srv/src`.

45. **Assert the closure, not the rollup.** (`powershell-module-cli-tools.md`)
    → A created `repositories/{repo_id}` directory is not evidence; assert real
    refs/objects and a resolvable HEAD.

46. **DB writes triggered by a state change ride the caller's transaction.**
    (`issue-status-side-effects.md`, constitution V)
    → Keep adoption DB-free apart from the placement reason it already owns.

47. **Advisory diagnostics degrade; core operations fail the transaction.**
    (`issue-status-side-effects.md`)
    → Failing to compute dirty counts must not fail the repair.

## F. Deployment loop and rollout

48. **`/srv/src/<repo>` is triple-purposed — poll target, app Git workspace, and
    build input — and `git-projects-update` hard-resets it to `origin/<ref>`
    roughly every 15 minutes.** (`wiki/self-hosted-deployment.md`,
    `deployment-checkout-recovery.md`)
    → Cluster worktrees must not depend on state that lives only there. `git reset
    --hard` does not delete branches, which bounds the window, but nothing else
    protects it.

49. **`git-projects-fix-permissions` chmod-sweeps `/srv/src`** (it historically
    stripped `+x` off deployed binaries). (`wiki/self-hosted-deployment.md`)
    → Confirm no equivalent sweep covers `{shared_root}`.

50. **The generalised lesson of the `/srv/src` incident: "All three failure modes
    go away when artifacts live outside `/srv/src`."**
    (`wiki/self-hosted-deployment.md`)
    → Direct prior-art endorsement of moving VK's Git state into app-owned
    storage.

51. **`origin` inside a clone-of-a-clone points at the local path and resolves to
    a possibly stale branch.**
    (`homelab/knowledge-base/cloudflare-access-service-token-live-enablement.md`)
    → `git clone --bare /srv/src/<repo>` gives the store an `origin` pointing at
    the coordinator's checkout. Set the real remote URLs explicitly.

52. **[H] The deploy loop restarts services and rolls back on a failed health
    probe.** (`homelab/docs/knowledge-base/vibe-kanban-deploy-loop-and-ntfy.md`)
    → A rollback restores a binary *without* the resolver while worktrees are
    already repointed at the store. That old binary would find the branch missing
    in `repo.path` and fall through to destructive recreation. The change needs a
    rollback-safe story, not just a forward one.

53. **[H] The deploy build worktree (`/srv/src/vibe-kanban-rebuild-cache/build-tree`)
    is deliberately left behind and removed only at the start of the next build.**
    (`vibe-kanban-deploy-loop-and-ntfy.md`)
    → Any "scan all worktrees" sweep must exclude it. (This task's sweep only
    walks `{shared_root}/workspaces`, so it is naturally excluded — state that.)

54. **`vk-deploy-notify` is a homelab-side systemd/Nix helper, not a Rust API.**
    It lives in `homelab/modules/vibe-kanban-rebuild.nix` and appears nowhere in
    `crates/`; it is null-topic-safe, and the homelab convention is to reuse it
    rather than mint a new ntfy topic. (`vibe-kanban-deploy-loop-and-ntfy.md`)
    → Recorded as a fact about the deployment, **not** an instruction for this
    change: that module is out of scope, so nothing here can call it. The
    portability sweep reports through structured logging and the placement
    reason, both of which are in-process and already surfaced.

55. **Runtime-written paths need a tmpfiles rule *and* a `ReadWritePaths` entry;
    `ProtectSystem=strict` denies writes even with correct Unix ownership. A
    worker booting before the coordinator created the directory must degrade, not
    fail to start.** (`vibe-kanban-remote-attachment-storage.md`,
    `homelab/knowledge-base/ohana-data-platform-management.md`)
    → `{shared_root}` is already in `ReadWritePaths`, so `repositories/` inherits
    it — verify rather than assume.

56. **Expect unrelated red: `ci/check-project-context.sh` fails inside VK
    worktrees because it assumes sibling checkouts.**
    (`personal-web-tool-subdomain-recipe.md`)

57. **Placement is the cheapest correctness control: put the actor on the host
    that owns the state.** (`ohana-data-platform-management.md`)
    → Endorses coordinator-owned administration; argues against workers fetching,
    gc-ing or pruning the store.

## G. Verification

58. **The acceptance gate is a two-node deployment exercise — disconnect the
    coordinator, cancel a process group, remove the shared mount, verify worktree
    integrity. "Passing local tests does not replace that deployment gate."**
    (`clustered-workspace-execution.md`)

59. **Test against fixture roots, never the live worktrees; cover none, all and
    *partial*.** (`worktree-formatting-prerequisites.md`)

60. **Add the new module and its tests to the CI path filters — "adding a test
    command to a filtered job is insufficient if changes to the tested files do
    not trigger that job."** (`worktree-formatting-prerequisites.md`)
    → `crates/git/**` is *already* in the `backend` filter
    (`.github/workflows/test.yml:64`). Genuinely missing, and holding most of
    this change and most of its tests: `crates/workspace-manager/**`,
    `crates/worktree-manager/**`, `crates/worker/**`,
    `crates/cluster-protocol/**`. A pre-existing hole.

61. **A repair path's exit code is not the verification; run the independent
    probe as the worker's service identity.** (`deployment-checkout-recovery.md`,
    `vk-app-managed-cli-tools.md`)

---

## Open contradiction to resolve in the spec

**Two object stores for one repository.** `wiki/self-hosted-deployment.md`
records that registering task worktrees under `/srv/src/<repo>/.git/worktrees/`
was an *accepted design*, not an accident. Routing cluster-placed workspaces at a
separate store bifurcates the model: a branch created in one store is invisible
in the other. The spec must state explicitly what happens when a workspace is
created locally and later placed on the cluster (today: placement is immutable
after reservation and `local` never becomes remote, so the transition does not
exist), and how a branch pushed by hand into `/srv/src/<repo>` reaches the store
(today: `ensure` fetches `+refs/heads/*` from `repo.path` on every provisioning).
