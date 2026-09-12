//! The Git store a clustered workspace's worktrees are actually backed by.
//!
//! A linked worktree records the absolute path of the repository that owns it.
//! When that repository is the operator's checkout — `/srv/src/<repo>` on the
//! coordinator — the path resolves on the coordinator and nowhere else, so every
//! Git command in the worktree fails on the worker that was given the work. The
//! fix is not to translate the path but to move the thing it names: each
//! repository gets a bare store under the shared root, at a location every node
//! derives identically from the repository's id.
//!
//! Two operations live here. [`SharedRepositoryStore::ensure`] materialises the
//! store for a repository, and [`SharedRepositoryStore::adopt`] re-links a
//! worktree that was created against the old, node-local path. Adoption touches
//! only pointer files: the worktrees it repairs hold agent edits, untracked
//! build output, and — where an agent managed to commit before the breakage —
//! commits reachable from nowhere else.

use std::path::{Path, PathBuf};

use db::models::repo::Repo;
use git::GitCli;
use tracing::{debug, info, warn};
use utils::worktree_linkage::{LinkageStatus, WorktreeLinkage};
use uuid::Uuid;
use worktree_manager::RepositoryAdminLockManager;

use crate::workspace_manager::{SharedWorkspacePaths, WorkspaceError};

/// Prefix for the per-attempt staging directory a store is built in before it is
/// renamed into place. Leading dot so a partially built store is not mistaken
/// for a repository id.
const STAGING_PREFIX: &str = ".";
const STAGING_SUFFIX: &str = ".incoming";

/// Remote that points back at the operator's registered checkout. Kept alongside
/// the real remotes so branches created by coordinator-local workspaces remain
/// reachable, and so the store can be seeded without network access.
const REGISTERED_REMOTE: &str = "vk-registered";
const ORIGIN_FETCH_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";

/// What [`SharedRepositoryStore::adopt`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptOutcome {
    /// The worktree already resolved inside the shared root. Nothing was done.
    AlreadyPortable,
    /// The worktree was re-linked to the store.
    Adopted { common_dir: PathBuf },
    /// Deliberately left alone, with the reason.
    Skipped { reason: String },
}

/// Coordinator-owned Git stores on shared storage, one per repository.
#[derive(Clone)]
pub struct SharedRepositoryStore {
    paths: SharedWorkspacePaths,
    locks: RepositoryAdminLockManager,
}

impl SharedRepositoryStore {
    pub fn new(
        shared_root: impl Into<PathBuf>,
        locks: RepositoryAdminLockManager,
    ) -> Result<Self, WorkspaceError> {
        Ok(Self {
            paths: SharedWorkspacePaths::new(shared_root)?,
            locks,
        })
    }

    /// Where a repository's store lives. Derived from the id, never configured,
    /// so every node computes the same path without agreeing on anything.
    pub fn path_for(&self, repo_id: Uuid) -> PathBuf {
        self.paths.repository_dir(repo_id)
    }

    /// Materialise the store for `repo` and guarantee it resolves `target_branch`.
    ///
    /// Idempotent, and cheap when the store is already good, because it runs on
    /// every provisioning of every cluster workspace.
    pub async fn ensure(
        &self,
        repo: &Repo,
        target_branch: &str,
    ) -> Result<PathBuf, WorkspaceError> {
        let store = self.path_for(repo.id);

        // Cheap path first: an existing store that already holds the branch
        // needs nothing. Proven with `cat-file -e`, not `rev-parse`.
        if Self::store_resolves(&store, target_branch)? {
            Self::ensure_origin_fetch_refspec(&GitCli::new(), &store)?;
            debug!(
                repo = %repo.name,
                store = %store.display(),
                "shared repository store already resolves the target branch"
            );
            let _ = self.mirror_branch_back(repo, target_branch);
            return Ok(store);
        }

        // The clone deliberately runs *outside* the administration lease. That
        // lease is a bounded SQLite lease, and cloning a large repository onto
        // network storage can outlive it — which would silently unfence the very
        // operation the lease exists to fence. The staging path carries a fresh
        // id per attempt, so two racing clones cannot corrupt each other, and the
        // loser's work is discarded when it sees the winner's store.
        let staged = if Self::is_store(&store)? {
            None
        } else {
            Some(self.clone_into_staging(repo).await?)
        };

        // Lock on the store itself, not on `repositories/`. Worktree creation
        // and cleanup fence on the store path, and `canonical_lock_key` derives
        // the in-process mutex from the path — so locking a different path here
        // would leave those callers unserialized against this one in-process
        // while still contending for the same per-repository database lease,
        // turning a concurrent provisioning into a spurious "lock busy" failure.
        let guard = self.locks.acquire(repo.id, &store).await;
        let guard = match guard {
            Ok(guard) => guard,
            Err(e) => {
                // Do not leave a full bare clone behind on shared storage.
                if let Some(staged) = &staged {
                    let _ = std::fs::remove_dir_all(staged);
                }
                return Err(e.into());
            }
        };
        let published = self.publish_and_fetch(repo, target_branch, &store, staged);
        // Release the lease whether or not the work succeeded; a lease held past
        // its critical section is worse than no lease.
        guard.release().await?;
        published?;

        let _ = self.mirror_branch_back(repo, target_branch);
        Ok(store)
    }

    /// Re-link a worktree whose `.git` points at a repository this node cannot
    /// reach, without touching a single file in the working tree.
    pub async fn adopt(
        &self,
        repo: &Repo,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<AdoptOutcome, WorkspaceError> {
        let shared_root = self.paths.root().to_path_buf();
        let status = WorktreeLinkage::probe(worktree_path, &shared_root);

        match &status {
            LinkageStatus::Portable { .. } => return Ok(AdoptOutcome::AlreadyPortable),
            LinkageStatus::OwnRepository => {
                return Ok(AdoptOutcome::Skipped {
                    reason: "the directory is an ordinary repository, not a linked worktree"
                        .to_string(),
                });
            }
            LinkageStatus::Missing => {
                return Ok(AdoptOutcome::Skipped {
                    reason: "the directory has no .git entry".to_string(),
                });
            }
            LinkageStatus::Indeterminate { reason } => {
                // Unknown is not broken. Repairing on a guess here would point a
                // live workspace at a repository that is not its own.
                return Ok(AdoptOutcome::Skipped {
                    reason: format!("linkage could not be determined: {reason}"),
                });
            }
            LinkageStatus::Dangling { .. }
            | LinkageStatus::OutsideSharedRoot { .. }
            | LinkageStatus::BackPointerMismatch { .. } => {}
        }

        let store = self.path_for(repo.id);
        let cli = GitCli::new();

        // Everything that can refuse, refuses before the first mutation. A
        // half-adopted worktree — new pointer, absent objects — is worse than an
        // unadopted one, because it looks repaired.
        if !Self::is_store(&store)? {
            return Ok(AdoptOutcome::Skipped {
                reason: format!("no shared store at {} to adopt into", store.display()),
            });
        }
        // A *local* head, specifically. `write_linkage` below writes
        // `ref: refs/heads/{branch}` into the worktree's HEAD, so a guard
        // satisfied by a same-named remote-tracking ref would point a live
        // worktree at an unborn branch — and the `git reset -q` that follows
        // would then clear the index instead of rebuilding it, making every
        // tracked file in someone's work-in-progress read as deleted, while
        // this function reported `Adopted`. The guard and the mutation must
        // agree on what "present" means.
        if !Self::local_branch_commit_present(&cli, &store, branch)? {
            return Ok(AdoptOutcome::Skipped {
                reason: format!(
                    "branch '{branch}' and its commits are not present in {}; \
                     adopting would discard them",
                    store.display()
                ),
            });
        }
        if let Some(holder) =
            Self::branch_checked_out_elsewhere(&cli, &store, branch, worktree_path)?
        {
            return Ok(AdoptOutcome::Skipped {
                reason: format!(
                    "branch '{branch}' is already checked out by {}",
                    holder.display()
                ),
            });
        }

        // `info!`, before acting, naming the target and why it was chosen: this
        // rewrites the linkage of a directory holding someone's unsaved work.
        info!(
            repo = %repo.name,
            worktree = %worktree_path.display(),
            store = %store.display(),
            "adopting worktree into the shared repository store because {}",
            status.describe()
        );

        let admin_name = Self::admin_dir_name(worktree_path);
        let admin_dir = store.join("worktrees").join(&admin_name);
        Self::write_linkage(&admin_dir, worktree_path, branch)?;

        // Let git normalise both directions, then verify independently — a zero
        // exit is not evidence.
        if let Err(e) = cli.worktree_repair(&store, &[worktree_path]) {
            warn!(
                worktree = %worktree_path.display(),
                "git worktree repair reported an error after adoption: {e}"
            );
        }

        // Rebuild the index. Without this the worktree reports every tracked
        // file as simultaneously deleted and untracked, because the index that
        // used to live in the unreachable administration directory is gone.
        // `reset` with no arguments is mixed: it touches no working-tree file.
        if let Err(e) = cli.git(worktree_path, ["reset", "-q"]) {
            return Err(WorkspaceError::PartialCreation(format!(
                "adopted {} but could not rebuild its index: {e}",
                worktree_path.display()
            )));
        }

        match WorktreeLinkage::probe(worktree_path, &shared_root) {
            LinkageStatus::Portable { common_dir } => {
                info!(
                    worktree = %worktree_path.display(),
                    "adopted; now resolves to {}",
                    common_dir.display()
                );
                Ok(AdoptOutcome::Adopted { common_dir })
            }
            other => Err(WorkspaceError::WorktreeNotPortable {
                repo_name: repo.name.clone(),
                detail: format!("after adoption the worktree {}", other.describe()),
            }),
        }
    }

    // ---- internals ----

    fn staging_dir(&self, repo_id: Uuid) -> PathBuf {
        self.paths.repositories_dir().join(format!(
            "{STAGING_PREFIX}{repo_id}.{}{STAGING_SUFFIX}",
            Uuid::new_v4()
        ))
    }

    async fn clone_into_staging(&self, repo: &Repo) -> Result<PathBuf, WorkspaceError> {
        let staging = self.staging_dir(repo.id);
        let repositories = self.paths.repositories_dir();
        let source = repo.path.clone();
        let staging_for_task = staging.clone();

        tokio::task::spawn_blocking(move || -> Result<(), WorkspaceError> {
            std::fs::create_dir_all(&repositories)?;
            Self::make_group_writable(&repositories);
            GitCli::new()
                .clone_bare(&source, &staging_for_task)
                .map_err(|e| {
                    WorkspaceError::PartialCreation(format!(
                        "could not clone {} into the shared store: {e}",
                        source.display()
                    ))
                })
        })
        .await
        .map_err(|e| WorkspaceError::PartialCreation(format!("clone task failed: {e}")))??;

        Ok(staging)
    }

    fn publish_and_fetch(
        &self,
        repo: &Repo,
        target_branch: &str,
        store: &Path,
        staged: Option<PathBuf>,
    ) -> Result<(), WorkspaceError> {
        let cli = GitCli::new();

        if let Some(staged) = staged {
            if Self::is_store(store)? {
                // Somebody else published while we were cloning. Theirs is as
                // good as ours; discard our staging rather than racing a rename.
                let _ = std::fs::remove_dir_all(&staged);
            } else {
                Self::configure(&cli, &staged, repo)?;
                // Publication is a rename, so the store becomes visible complete
                // or not at all.
                std::fs::rename(&staged, store).map_err(|e| {
                    let _ = std::fs::remove_dir_all(&staged);
                    WorkspaceError::PartialCreation(format!(
                        "could not publish the shared store at {}: {e}",
                        store.display()
                    ))
                })?;
                Self::make_group_writable(store);
                info!(
                    repo = %repo.name,
                    store = %store.display(),
                    "created shared repository store"
                );
            }
        }

        // Bring the branches this workspace might need into the store. A remote
        // that cannot be reached is tolerated; the target branch resolving
        // afterwards is not optional.
        //
        // Both namespaces, because a target branch may name either. The picker
        // offered whatever `get_all_branches` read from this same checkout, so
        // copying the checkout's refs verbatim makes the set of branches a user
        // can pick and the set this store can serve the same set, by
        // construction — and as fresh as that checkout, which is the freshness
        // the user was shown.
        //
        // Deliberately *not* done by giving the store its own `origin` fetch
        // refspec: `configure` retargets `origin` at the forge, so a store that
        // populated `refs/remotes/origin/*` itself would carry a second,
        // differently-fresh notion of `origin/main`.
        //
        // Two invocations, not one, and this is load-bearing rather than
        // stylistic. `git fetch` is atomic across its refspecs, so one refused
        // refspec discards the writes of every other. The heads refspec is
        // refused whenever the store has a worktree checked out on a branch the
        // checkout also holds — `refusing to fetch into branch '…' checked out
        // at …` — which is reached through `heal_cluster_worktree`, since that
        // path calls `ensure` with the *workspace* branch and so mirrors `vk/…`
        // back into the checkout. Batched, that refusal would silently discard
        // the remote-tracking mirror, the one this change exists to add.
        // Separately, each namespace fails on its own and the other still lands.
        //
        // Additive: force-update, never `--prune`. These namespaces are shared
        // by every workspace of this repository on every node.
        for refspec in [
            "+refs/heads/*:refs/heads/*",
            "+refs/remotes/*:refs/remotes/*",
        ] {
            if let Err(e) = cli.fetch_with_refspec(store, &repo.path.to_string_lossy(), refspec) {
                debug!(
                    repo = %repo.name,
                    "could not mirror {refspec} from the registered checkout: {e}"
                );
            }
        }
        // Why a fetch could not be attempted, if any was tried and failed. The
        // refusal at the end reads very differently when it is "the branch is
        // not there" versus "we never managed to ask".
        let mut failed_remotes: Vec<String> = Vec::new();
        if !Self::branch_commit_present(&cli, store, target_branch)?
            && let Ok(remotes) = cli.list_remotes(store)
        {
            // The checkout mirror above covers the ordinary case. This reaches
            // the real forge for the one it does not: a branch that exists
            // upstream and that the coordinator's checkout has never fetched.
            // If the target branch is prefixed with a remote's name, only that
            // remote can hold it. Asking the others would send the
            // local-to-local form — `+refs/heads/upstream/main:refs/heads/upstream/main`
            // to `origin` — and if that remote happens to hold a branch
            // literally named `upstream/main`, it lands as a *local* head in the
            // shared store, where local-first resolution then prefers it forever,
            // at the wrong commit, for every workspace on every node.
            let owning_remote = remotes
                .iter()
                .find(|(name, _)| target_branch.starts_with(&format!("{name}/")))
                .map(|(name, _)| name.clone());
            let mut fetch_failures = Vec::new();
            for (name, url) in remotes {
                if name == REGISTERED_REMOTE {
                    continue;
                }
                if let Some(owner) = &owning_remote
                    && &name != owner
                {
                    continue;
                }
                let refspec = Self::fallback_refspec(&name, target_branch);
                // Recorded, not discarded. The refusal below asserts the branch
                // is not present; if the only attempt to obtain it never ran —
                // expired forge credentials, an unreachable host,
                // `GIT_TERMINAL_PROMPT=0` declining a prompt — then that
                // assertion misdirects the very investigation this message
                // exists to shorten.
                if let Err(e) = cli.fetch_with_refspec(store, &url, &refspec) {
                    debug!(
                        repo = %repo.name,
                        "could not fetch '{target_branch}' from remote '{name}': {e}"
                    );
                    fetch_failures.push(format!("{name}: {}", Self::summarize(&e.to_string())));
                }
                // A zero exit is not evidence that the ref we need now exists;
                // ask the store.
                if Self::branch_commit_present(&cli, store, target_branch)? {
                    break;
                }
            }
            failed_remotes = fetch_failures;
        }

        match Self::resolved_branch_ref(&cli, store, target_branch)? {
            Some(reference) => {
                debug!(
                    repo = %repo.name,
                    store = %store.display(),
                    "target branch '{target_branch}' resolves to {reference}"
                );
                Ok(())
            }
            None => Err(WorkspaceError::SharedStore {
                repo_name: repo.name.clone(),
                branch: target_branch.to_string(),
                detail: if failed_remotes.is_empty() {
                    format!(
                        "no branch of that name is present in {}, as either a \
                         local or a remote-tracking ref",
                        store.display()
                    )
                } else {
                    format!(
                        "no branch of that name is present in {}, and fetching \
                         it failed ({})",
                        store.display(),
                        failed_remotes.join("; ")
                    )
                },
            }),
        }
    }

    /// Reduce a git failure to one bounded line fit for an API response body.
    ///
    /// `GitCliError::CommandFailed` carries the subprocess's combined output
    /// verbatim, and that string is partly remote-controlled — a forge can emit
    /// arbitrary `remote:` lines, and a credential helper writes its own
    /// diagnostics to stderr. Since this text now reaches the user rather than
    /// only the log, take the first meaningful line and cap it: an operator
    /// needs the reason, not a transcript, and an error body is the wrong place
    /// to relay unbounded third-party output.
    fn summarize(message: &str) -> String {
        const LIMIT: usize = 200;
        let line = message
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("no output");
        match line.char_indices().nth(LIMIT) {
            Some((cut, _)) => format!("{}…", &line[..cut]),
            None => line.to_string(),
        }
    }

    /// The refspec that could actually bring `target_branch` into the store
    /// from the remote called `remote_name`.
    ///
    /// A remote-prefixed target names a branch that upstream knows under a
    /// different name: `origin/main` is upstream's `main`, and it belongs in the
    /// store's remote-tracking namespace where `resolved_branch_ref` will look
    /// for it. Asking a forge for `refs/heads/origin/main` — which is what this
    /// used to do — is a network round trip that cannot succeed, paid once per
    /// repository on the workspace-creation request.
    ///
    /// A target that is not prefixed with *this* remote's name is left in the
    /// original local-to-local form: nothing about it says this remote holds a
    /// branch under some other name, and guessing would be worse than the
    /// bounded attempt that already happens.
    fn fallback_refspec(remote_name: &str, target_branch: &str) -> String {
        match target_branch.strip_prefix(&format!("{remote_name}/")) {
            Some(upstream_branch) if !upstream_branch.is_empty() => {
                format!("+refs/heads/{upstream_branch}:refs/remotes/{target_branch}")
            }
            _ => format!("+refs/heads/{target_branch}:refs/heads/{target_branch}"),
        }
    }

    /// Configure a freshly cloned store, before it is published and therefore
    /// before any worktree can be registered in it.
    fn configure(cli: &GitCli, store: &Path, repo: &Repo) -> Result<(), WorkspaceError> {
        let cfg = |key: &str, value: &str| -> Result<(), WorkspaceError> {
            cli.set_config(store, key, value).map_err(|e| {
                WorkspaceError::PartialCreation(format!("could not set {key} on the store: {e}"))
            })
        };

        // Automatic maintenance is disabled *before* the store is published,
        // because `git gc --auto` fires opportunistically on ordinary commands
        // and prunes worktree registrations. Once workspaces exist, a routine
        // `git status` run by a worker could otherwise unregister a different
        // workspace — including one owned by another node.
        cfg("gc.auto", "0")?;
        cfg("gc.autoDetach", "false")?;
        cfg("maintenance.auto", "false")?;
        cfg("gc.worktreePruneExpire", "never")?;
        cfg("core.logAllRefUpdates", "true")?;
        // New objects and lock files are created by whichever node ran the
        // command, so the group must be able to reopen them.
        cfg("core.sharedRepository", "group")?;

        // `git clone --bare` points `origin` at whatever it cloned from — here,
        // the coordinator's local checkout. Left alone, `origin/main` in the
        // store would mean "the checkout's main", not the forge's, and pushes
        // and pull requests would go to a directory. Copy the registered
        // repository's real remotes over it, and keep the checkout under its own
        // name so it stays fetchable.
        let registered = repo.path.to_string_lossy().to_string();
        cli.set_remote_url(store, REGISTERED_REMOTE, &registered)
            .map_err(|e| {
                WorkspaceError::PartialCreation(format!("could not record the seed remote: {e}"))
            })?;
        if let Ok(remotes) = cli.list_remotes(&repo.path) {
            for (name, url) in remotes {
                cli.set_remote_url(store, &name, &url).map_err(|e| {
                    WorkspaceError::PartialCreation(format!(
                        "could not copy remote '{name}' to the store: {e}"
                    ))
                })?;
            }
        }
        Self::ensure_origin_fetch_refspec(cli, store)?;

        Ok(())
    }

    /// Give libgit2 the mapping it needs to associate `origin/*` refs with the
    /// `origin` remote. URL-only remotes work for explicit fetches and pushes,
    /// but `git_branch_remote_name` cannot resolve them for pull-request flows.
    fn ensure_origin_fetch_refspec(cli: &GitCli, store: &Path) -> Result<(), WorkspaceError> {
        // A store without origin (for example, a registered local-only repo)
        // has nothing to repair.
        if cli.get_remote_url(store, "origin").is_err() {
            return Ok(());
        }

        let configured = cli
            .git(store, ["config", "--get-all", "remote.origin.fetch"])
            .unwrap_or_default();
        if configured.lines().any(|line| line == ORIGIN_FETCH_REFSPEC) {
            return Ok(());
        }

        cli.git(
            store,
            [
                "config",
                "--add",
                "remote.origin.fetch",
                ORIGIN_FETCH_REFSPEC,
            ],
        )
        .map_err(|e| {
            WorkspaceError::PartialCreation(format!(
                "could not configure the origin fetch refspec on {}: {e}",
                store.display()
            ))
        })?;
        Ok(())
    }

    /// Best-effort: keep the workspace branch present in the registered
    /// checkout too.
    ///
    /// This exists for rollback. A release without this change resolves
    /// branch-scoped operations against `repo.path`, so a branch that lives only
    /// in the store would look missing to it. Failure is advisory — the store is
    /// the authority, and refusing to provision because a courtesy mirror failed
    /// would trade a working workspace for a tidy one.
    fn mirror_branch_back(&self, repo: &Repo, branch: &str) -> Result<(), WorkspaceError> {
        let store = self.path_for(repo.id);
        let cli = GitCli::new();
        // Local heads only: `GitCli::push` sends
        // `refs/heads/{branch}:refs/heads/{branch}`, so a match on a
        // remote-tracking ref would spawn a push whose source ref does not
        // exist — a guaranteed failure, once per repository per provisioning,
        // swallowed into a debug line. There is also nothing to mirror back: a
        // remote-tracking ref came *from* the checkout.
        if !Self::local_branch_commit_present(&cli, &store, branch).unwrap_or(false) {
            return Ok(());
        }
        if let Err(e) = cli.push(&store, &repo.path.to_string_lossy(), branch, false) {
            debug!(
                repo = %repo.name,
                "could not mirror branch '{branch}' back to the registered checkout: {e}"
            );
        }
        Ok(())
    }

    /// Whether `ensure` can return without doing anything.
    ///
    /// Local heads only, and this is the difference between a fresh workspace
    /// and a silently stale one. A remote-tracking ref is a copy of a branch
    /// that moves upstream: once `refs/remotes/origin/main` exists, accepting it
    /// here would skip the mirror on every later provisioning and freeze the
    /// store at whatever commit the first one captured — so every workspace
    /// after the first would branch from a stale target while the picker showed
    /// the current one. Since `origin/main` is the default target branch, that
    /// would be the common case, not the corner.
    ///
    /// A local head still short-circuits, exactly as before this change. That
    /// carries its own staleness for a local target branch, which is pre-existing
    /// behaviour and deliberately left alone here.
    fn store_resolves(store: &Path, branch: &str) -> Result<bool, WorkspaceError> {
        if !Self::is_store(store)? {
            return Ok(false);
        }
        Self::local_branch_commit_present(&GitCli::new(), store, branch)
    }

    /// A directory is a store only if it holds an object database. An empty or
    /// half-written directory is not evidence of anything.
    fn is_store(store: &Path) -> Result<bool, WorkspaceError> {
        Ok(std::fs::exists(store.join("objects"))?)
    }

    /// The ref a workspace's target branch names in the store, or `None` when
    /// it names nothing here.
    ///
    /// A target branch is not necessarily a local branch name. The picker that
    /// produced it lists branches under the names `git::get_all_branches`
    /// returns — `origin/main` for a remote-tracking branch, `main` for a local
    /// one — and `origin/main` is the default it applies when a repository has
    /// no configured `default_target_branch`, which is most of them. So the
    /// order here is local, then remote-tracking: the same order, with the same
    /// outcome, as `GitService::find_branch`, which is what validated the
    /// user's choice against the registered checkout in the first place.
    /// Resolving it any other way here would mean the store could not serve the
    /// branch the rest of the product already accepted.
    ///
    /// Presence is proven with `commit_exists` (`cat-file -e`) for both forms:
    /// a ref that names an object the store does not hold is not a branch this
    /// store can serve. Reusing one probe also keeps one failure direction —
    /// `CommandFailed` means absent, anything else propagates.
    ///
    /// The two refs are spelled out rather than left to git's own revision
    /// precedence, which would resolve the bare name for us. That precedence is
    /// wider than `find_branch`: it also accepts `refs/tags/<name>` and
    /// `refs/<name>`, so a repository with a tag named `main` would resolve a
    /// target branch `main` to the tag. Naming the two namespaces keeps this
    /// answering the same question `find_branch` answers, which is what
    /// `branch_resolution_agrees_with_git_services_branch_lookup` pins.
    fn resolved_branch_ref(
        cli: &GitCli,
        store: &Path,
        branch: &str,
    ) -> Result<Option<String>, WorkspaceError> {
        for reference in [
            format!("refs/heads/{branch}"),
            format!("refs/remotes/{branch}"),
        ] {
            let present = cli.commit_exists(store, &reference).map_err(|e| {
                WorkspaceError::PartialCreation(format!("could not query the store: {e}"))
            })?;
            if present {
                return Ok(Some(reference));
            }
        }
        Ok(None)
    }

    fn branch_commit_present(
        cli: &GitCli,
        store: &Path,
        branch: &str,
    ) -> Result<bool, WorkspaceError> {
        Ok(Self::resolved_branch_ref(cli, store, branch)?.is_some())
    }

    /// Whether the store holds `branch` as a **local head** specifically.
    ///
    /// Distinct from [`Self::branch_commit_present`], and the distinction
    /// matters in three places. A remote-tracking ref is a *copy* of something
    /// that moves upstream, so it is never evidence that this store is up to
    /// date; a workspace branch is always a local head, so anything that acts on
    /// one — pushing it back to the checkout, re-linking a worktree to it — must
    /// not be satisfied by a same-named remote-tracking ref.
    fn local_branch_commit_present(
        cli: &GitCli,
        store: &Path,
        branch: &str,
    ) -> Result<bool, WorkspaceError> {
        Ok(Self::resolved_branch_ref(cli, store, branch)?
            .is_some_and(|reference| reference.starts_with("refs/heads/")))
    }

    /// A branch may be checked out by at most one worktree. Adopting a branch
    /// another workspace holds would break that workspace, so it is a refusal.
    fn branch_checked_out_elsewhere(
        cli: &GitCli,
        store: &Path,
        branch: &str,
        worktree_path: &Path,
    ) -> Result<Option<PathBuf>, WorkspaceError> {
        let entries = cli.list_worktrees(store).map_err(|e| {
            WorkspaceError::PartialCreation(format!("could not list worktrees of the store: {e}"))
        })?;
        for entry in entries {
            if entry.branch.as_deref() != Some(branch) {
                continue;
            }
            let holder = PathBuf::from(&entry.path);
            let same = std::fs::canonicalize(&holder).ok()
                == std::fs::canonicalize(worktree_path).ok()
                || holder == worktree_path;
            if !same {
                return Ok(Some(holder));
            }
        }
        Ok(None)
    }

    /// The name of the worktree's administration directory inside the store.
    ///
    /// Git would derive this from the path's basename, which is the repository
    /// name and therefore identical for every workspace of a repository; it
    /// disambiguates with a numeric suffix that depends on creation order. That
    /// is fine for git, which resolves registrations by path, but useless for a
    /// repair that has to be re-runnable and identical on every node. The
    /// workspace directory's own name is unique and stable, so use it.
    fn admin_dir_name(worktree_path: &Path) -> String {
        let repo = worktree_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let workspace = worktree_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "workspace".to_string());
        format!("{workspace}-{repo}")
    }

    /// Write both halves of the two-way link.
    ///
    /// Both files are written to a sibling temporary path and renamed over the
    /// target, so a reader never sees a partial pointer. The worktree's `.git`
    /// marker in particular is never unlinked first: a directory with no `.git`
    /// anywhere under it is classified as holding no work, and a cleanup pass
    /// that ran in that window would delete it.
    fn write_linkage(
        admin_dir: &Path,
        worktree_path: &Path,
        branch: &str,
    ) -> Result<(), WorkspaceError> {
        std::fs::create_dir_all(admin_dir)?;
        let git_file = worktree_path.join(".git");

        atomic_write(&admin_dir.join("commondir"), "../..\n")?;
        atomic_write(
            &admin_dir.join("gitdir"),
            &format!("{}\n", git_file.display()),
        )?;
        atomic_write(
            &admin_dir.join("HEAD"),
            &format!("ref: refs/heads/{branch}\n"),
        )?;
        atomic_write(&git_file, &format!("gitdir: {}\n", admin_dir.display()))?;
        Ok(())
    }

    /// Make a directory group-writable and setgid so files created inside it by
    /// another node's process stay reachable to this group.
    ///
    /// Setgid propagates group *ownership*; it does not set the mode of files a
    /// worker creates, which follows that process's umask. The store's
    /// `core.sharedRepository=group` covers what git itself creates.
    fn make_group_writable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                let mut perms = metadata.permissions();
                perms.set_mode(perms.mode() | 0o2775);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
        #[cfg(not(unix))]
        let _ = path;
    }
}

/// Replace a file's contents without ever exposing a partial one: write a
/// sibling temporary in the same directory, then rename over the target.
fn atomic_write(target: &Path, contents: &str) -> Result<(), WorkspaceError> {
    let parent = target.parent().unwrap_or(Path::new("."));
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "vk".to_string()),
        Uuid::new_v4()
    ));
    // Write *and flush* before renaming. Rename is atomic with respect to the
    // directory entry, not to the file's contents: without the fsync, a crash
    // can leave the new name pointing at a zero-length file. An empty `.git` is
    // worse than a stale one — it reads as "this directory holds no work".
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    match std::fs::rename(&temporary, target) {
        Ok(()) => {
            // Flush the directory entry too, so the rename itself survives.
            if let Ok(dir) = std::fs::File::open(parent) {
                let _ = dir.sync_all();
            }
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(WorkspaceError::Io(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn cli() -> GitCli {
        GitCli::new()
    }

    /// A repository with one commit on `main`, standing in for the operator's
    /// registered checkout.
    fn seed_repo(path: &Path) {
        git::GitService::new()
            .initialize_repo_with_main_branch(path)
            .unwrap();
        // CI and this cluster's worker accounts have no global git identity.
        cli()
            .set_config(path, "user.email", "test@vibekanban.invalid")
            .unwrap();
        cli()
            .set_config(path, "user.name", "Vibe Kanban Test")
            .unwrap();
    }

    fn repo_record(name: &str, path: &Path) -> Repo {
        Repo {
            id: Uuid::new_v4(),
            path: path.to_path_buf(),
            name: name.to_string(),
            display_name: name.to_string(),
            setup_script: None,
            cleanup_script: None,
            archive_script: None,
            copy_files: None,
            parallel_setup_script: false,
            dev_server_script: None,
            default_target_branch: None,
            default_working_dir: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn admin_directory_names_are_unique_per_workspace_not_per_repository() {
        // Git would call both of these `repo` and `repo1`, in creation order.
        let a = Path::new("/shared/workspaces/aaaa/vibe-kanban");
        let b = Path::new("/shared/workspaces/bbbb/vibe-kanban");
        assert_ne!(
            SharedRepositoryStore::admin_dir_name(a),
            SharedRepositoryStore::admin_dir_name(b)
        );
        assert_eq!(
            SharedRepositoryStore::admin_dir_name(a),
            SharedRepositoryStore::admin_dir_name(a),
            "the name must be stable, so repair is re-runnable"
        );
    }

    #[test]
    fn atomic_write_replaces_without_an_intermediate_absence() {
        let fixture = TempDir::new().unwrap();
        let target = fixture.path().join(".git");
        fs::write(&target, "gitdir: /old\n").unwrap();

        atomic_write(&target, "gitdir: /new\n").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "gitdir: /new\n");
        let strays: Vec<_> = fs::read_dir(fixture.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "temporary files must not be left behind");
    }

    #[test]
    fn commit_presence_is_proven_not_assumed() {
        let fixture = TempDir::new().unwrap();
        let repo = fixture.path().join("repo");
        seed_repo(&repo);
        let store = fixture.path().join("store");
        cli().clone_bare(&repo, &store).unwrap();

        assert!(SharedRepositoryStore::branch_commit_present(&cli(), &store, "main").unwrap());
        assert!(
            !SharedRepositoryStore::branch_commit_present(&cli(), &store, "no-such-branch")
                .unwrap()
        );
        // `rev-parse` would echo this back; `cat-file -e` will not.
        assert!(
            !cli()
                .commit_exists(&store, "0123456789012345678901234567890123456789")
                .unwrap()
        );
    }

    #[test]
    fn a_directory_without_an_object_database_is_not_a_store() {
        let fixture = TempDir::new().unwrap();
        let empty = fixture.path().join("repositories").join("some-uuid");
        fs::create_dir_all(&empty).unwrap();

        assert!(
            !SharedRepositoryStore::is_store(&empty).unwrap(),
            "an existing directory is not evidence of a usable store"
        );
    }

    #[test]
    fn configure_disables_maintenance_and_retargets_origin() {
        let fixture = TempDir::new().unwrap();
        let repo = fixture.path().join("repo");
        seed_repo(&repo);
        cli()
            .set_remote_url(&repo, "origin", "https://example.invalid/org/repo.git")
            .unwrap();
        let store = fixture.path().join("store");
        cli().clone_bare(&repo, &store).unwrap();

        SharedRepositoryStore::configure(&cli(), &store, &repo_record("repo", &repo)).unwrap();

        let remotes: std::collections::HashMap<_, _> =
            cli().list_remotes(&store).unwrap().into_iter().collect();
        assert_eq!(
            remotes.get("origin").map(String::as_str),
            Some("https://example.invalid/org/repo.git"),
            "origin must name the forge, not the local checkout git clone --bare used"
        );
        assert_eq!(
            remotes.get(REGISTERED_REMOTE).map(String::as_str),
            Some(repo.to_string_lossy().as_ref()),
            "the registered checkout stays fetchable under its own name"
        );
        assert_eq!(
            cli()
                .git(&store, ["config", "--get-all", "remote.origin.fetch"])
                .unwrap()
                .trim(),
            ORIGIN_FETCH_REFSPEC
        );

        for (key, expected) in [
            ("gc.auto", "0"),
            ("gc.worktreePruneExpire", "never"),
            ("maintenance.auto", "false"),
        ] {
            let actual = cli().git(&store, ["config", "--get", key]).unwrap();
            assert_eq!(
                actual.trim(),
                expected,
                "{key} must be set before any worktree is registered"
            );
        }
    }

    #[test]
    fn origin_fetch_refspec_repair_is_idempotent() {
        let fixture = TempDir::new().unwrap();
        let repo = fixture.path().join("repo");
        seed_repo(&repo);
        cli()
            .set_remote_url(&repo, "origin", "https://example.invalid/org/repo.git")
            .unwrap();

        SharedRepositoryStore::ensure_origin_fetch_refspec(&cli(), &repo).unwrap();
        SharedRepositoryStore::ensure_origin_fetch_refspec(&cli(), &repo).unwrap();

        assert_eq!(
            cli()
                .git(&repo, ["config", "--get-all", "remote.origin.fetch"])
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec![ORIGIN_FETCH_REFSPEC]
        );
    }

    /// The production failure, end to end: a worktree created against a
    /// repository this node cannot see, re-linked into the shared store without
    /// losing committed, tracked or untracked content.
    #[tokio::test]
    async fn adopts_a_worktree_whose_repository_is_unreachable() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let node_local = fixture.path().join("srv-src");
        fs::create_dir_all(&shared_root).unwrap();
        fs::create_dir_all(&node_local).unwrap();

        let repo_path = node_local.join("repo");
        seed_repo(&repo_path);

        let workspace = shared_root.join("workspaces").join("ws-1");
        let worktree = workspace.join("repo");
        fs::create_dir_all(&workspace).unwrap();
        // Branch first, then add. `worktree add -b <b> <path> <b>` would ask git
        // to start the branch at itself; production creates the branch first for
        // the same reason.
        git::GitService::new()
            .create_branch(&repo_path, "vk/work", "main")
            .unwrap();
        cli()
            .worktree_add(&repo_path, &worktree, "vk/work", false)
            .unwrap();

        // Work the agent did before the breakage was noticed.
        fs::write(worktree.join("tracked.txt"), "agent edit\n").unwrap();
        cli().add_all(&worktree).unwrap();
        cli().commit(&worktree, "agent commit").unwrap();
        fs::create_dir_all(worktree.join("node_modules")).unwrap();
        fs::write(worktree.join("node_modules").join("marker"), "keep\n").unwrap();
        let committed = cli().resolve_commit(&worktree, "HEAD").unwrap().unwrap();

        // Build the store from the checkout, then make the checkout unreachable
        // — exactly what a worker sees.
        let repo = repo_record("repo", &repo_path);
        let store = shared_root.join("repositories").join(repo.id.to_string());
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        cli().clone_bare(&repo_path, &store).unwrap();
        cli()
            .fetch_with_refspec(
                &store,
                &repo_path.to_string_lossy(),
                "+refs/heads/*:refs/heads/*",
            )
            .unwrap();
        SharedRepositoryStore::configure(&cli(), &store, &repo).unwrap();
        fs::rename(&repo_path, node_local.join("moved-away")).unwrap();

        assert!(
            cli().git(&worktree, ["status", "--porcelain"]).is_err(),
            "precondition: the worktree is broken before adoption"
        );

        let outcome = store_for(&shared_root)
            .adopt(&repo, &worktree, "vk/work")
            .await
            .unwrap();

        assert!(
            matches!(outcome, AdoptOutcome::Adopted { .. }),
            "{outcome:?}"
        );
        assert_eq!(
            cli().resolve_commit(&worktree, "HEAD").unwrap().unwrap(),
            committed,
            "the commit made before the breakage must still be HEAD"
        );
        assert_eq!(
            fs::read_to_string(worktree.join("tracked.txt")).unwrap(),
            "agent edit\n"
        );
        assert!(
            worktree.join("node_modules").join("marker").exists(),
            "untracked files must survive"
        );
        let status = cli().git(&worktree, ["status", "--porcelain"]).unwrap();
        assert!(
            !status.contains("tracked.txt"),
            "the index must be rebuilt, not left absent: {status}"
        );

        // Idempotent.
        assert_eq!(
            store_for(&shared_root)
                .adopt(&repo, &worktree, "vk/work")
                .await
                .unwrap(),
            AdoptOutcome::AlreadyPortable
        );
    }

    #[tokio::test]
    async fn refuses_to_adopt_when_the_branch_is_not_in_the_store() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let node_local = fixture.path().join("srv-src");
        fs::create_dir_all(&shared_root).unwrap();
        fs::create_dir_all(&node_local).unwrap();

        let repo_path = node_local.join("repo");
        seed_repo(&repo_path);

        // Clone the store *before* the workspace branch exists, so the store
        // genuinely lacks its commits — the case where adopting would silently
        // move a workspace onto a history that is not its own.
        let repo = repo_record("repo", &repo_path);
        let store = shared_root.join("repositories").join(repo.id.to_string());
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        cli().clone_bare(&repo_path, &store).unwrap();

        let workspace = shared_root.join("workspaces").join("ws-1");
        let worktree = workspace.join("repo");
        fs::create_dir_all(&workspace).unwrap();
        git::GitService::new()
            .create_branch(&repo_path, "vk/work", "main")
            .unwrap();
        cli()
            .worktree_add(&repo_path, &worktree, "vk/work", false)
            .unwrap();
        fs::rename(&repo_path, node_local.join("moved-away")).unwrap();

        let outcome = store_for(&shared_root)
            .adopt(&repo, &worktree, "vk/work")
            .await
            .unwrap();

        match outcome {
            AdoptOutcome::Skipped { reason } => {
                assert!(reason.contains("not present"), "{reason}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(
            fs::read_to_string(worktree.join(".git"))
                .unwrap()
                .contains("srv-src"),
            "a refusal must not have rewritten the pointer"
        );
    }

    #[tokio::test]
    async fn leaves_a_healthy_worktree_and_an_ordinary_repository_alone() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        fs::create_dir_all(&shared_root).unwrap();

        let repo_path = shared_root.join("plain-repo");
        seed_repo(&repo_path);
        let repo = repo_record("repo", &repo_path);

        assert_eq!(
            store_for(&shared_root)
                .adopt(&repo, &repo_path, "main")
                .await
                .unwrap(),
            AdoptOutcome::Skipped {
                reason: "the directory is an ordinary repository, not a linked worktree"
                    .to_string()
            }
        );

        let absent = shared_root.join("empty");
        fs::create_dir_all(&absent).unwrap();
        assert_eq!(
            store_for(&shared_root)
                .adopt(&repo, &absent, "main")
                .await
                .unwrap(),
            AdoptOutcome::Skipped {
                reason: "the directory has no .git entry".to_string()
            }
        );
    }

    /// Build a store handle without a database. `adopt` never takes the
    /// administration lease (it is called by code that already holds the
    /// serialization it needs), so the manager is not exercised here.
    fn store_for(shared_root: &Path) -> SharedRepositoryStore {
        SharedRepositoryStore {
            paths: SharedWorkspacePaths::new(shared_root).unwrap(),
            locks: unreachable_locks(),
        }
    }

    /// A store handle whose administration lease actually works. `ensure`, unlike
    /// `adopt`, takes the lease, so it needs a pool holding the lock table.
    async fn store_with_locks(shared_root: &Path) -> SharedRepositoryStore {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            r#"
            CREATE TABLE repository_admin_locks (
                repo_id BLOB PRIMARY KEY NOT NULL,
                generation INTEGER NOT NULL,
                operation_id BLOB NOT NULL,
                acquired_at TEXT NOT NULL,
                lease_expires_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        SharedRepositoryStore {
            paths: SharedWorkspacePaths::new(shared_root).unwrap(),
            locks: RepositoryAdminLockManager::new(pool, std::time::Duration::from_secs(30))
                .unwrap(),
        }
    }

    /// A checkout shaped like `/srv/src/<repo>`: a local `main` **and** a
    /// remote-tracking `origin/main`, which is the name the create screen
    /// defaults a repository's target branch to.
    fn seed_checkout_with_remote(path: &Path) {
        seed_repo(path);
        // A second commit reachable only from origin/main, so a test that
        // silently substituted the local `main` would resolve to a different
        // commit and be caught.
        fs::write(path.join("upstream.txt"), "from upstream\n").unwrap();
        cli().add_all(path).unwrap();
        cli().commit(path, "upstream commit").unwrap();
        cli()
            .git(path, ["update-ref", "refs/remotes/origin/main", "HEAD"])
            .unwrap();
        // Rewind the local branch so the two genuinely differ.
        cli().git(path, ["reset", "--hard", "HEAD~1"]).unwrap();
        cli()
            .set_remote_url(path, "origin", "https://example.invalid/org/repo.git")
            .unwrap();
    }

    /// The failure the user reported: a workspace whose target branch is the
    /// create screen's default, `origin/main`, could not be provisioned at all.
    #[tokio::test]
    async fn ensure_serves_a_remote_prefixed_target_branch() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let checkout = fixture.path().join("srv-src").join("repo");
        fs::create_dir_all(&shared_root).unwrap();
        seed_checkout_with_remote(&checkout);
        let repo = repo_record("repo", &checkout);
        let upstream = cli()
            .resolve_commit(&checkout, "refs/remotes/origin/main")
            .unwrap()
            .unwrap();

        let store_handle = store_with_locks(&shared_root).await;
        let store = store_handle.ensure(&repo, "origin/main").await.unwrap();

        assert_eq!(
            SharedRepositoryStore::resolved_branch_ref(&cli(), &store, "origin/main").unwrap(),
            Some("refs/remotes/origin/main".to_string()),
            "a remote-prefixed target branch must resolve in the store"
        );
        assert_eq!(
            cli()
                .resolve_commit(&store, "refs/remotes/origin/main")
                .unwrap()
                .unwrap(),
            upstream,
            "it must resolve to the upstream commit, not the local branch's"
        );

        // The step that used to fail one frame later: the workspace branch is
        // created from the target branch *in the store*, through git2's
        // local-then-remote lookup.
        git::GitService::new()
            .create_branch(&store, "vk/work", "origin/main")
            .unwrap();
        let worktree = shared_root.join("workspaces").join("ws-1").join("repo");
        fs::create_dir_all(worktree.parent().unwrap()).unwrap();
        cli()
            .worktree_add(&store, &worktree, "vk/work", false)
            .unwrap();
        assert_eq!(
            cli().resolve_commit(&worktree, "HEAD").unwrap().unwrap(),
            upstream
        );
    }

    /// Mirroring is additive. `refs/remotes/*` in the store is shared by every
    /// workspace of this repository on every node, so a second `ensure` must not
    /// drop refs the registered checkout does not happen to have.
    #[tokio::test]
    async fn ensure_never_removes_refs_another_workspace_may_hold() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let checkout = fixture.path().join("srv-src").join("repo");
        fs::create_dir_all(&shared_root).unwrap();
        seed_checkout_with_remote(&checkout);
        let repo = repo_record("repo", &checkout);

        let store_handle = store_with_locks(&shared_root).await;
        let store = store_handle.ensure(&repo, "origin/main").await.unwrap();

        // Stand in for another live workspace's branch, and for a
        // remote-tracking ref the checkout has since dropped.
        cli()
            .git(
                &store,
                ["update-ref", "refs/heads/vk/other-workspace", "main"],
            )
            .unwrap();
        cli()
            .git(&store, ["update-ref", "refs/remotes/origin/gone", "main"])
            .unwrap();

        store_handle.ensure(&repo, "origin/main").await.unwrap();

        for reference in ["refs/heads/vk/other-workspace", "refs/remotes/origin/gone"] {
            assert!(
                cli().commit_exists(&store, reference).unwrap(),
                "{reference} must survive a re-run; mirroring is additive"
            );
        }
    }

    /// The steady state, which is where the first version of this fix silently
    /// stopped working.
    ///
    /// Once a workspace exists, the store has a worktree checked out on its
    /// branch and `mirror_branch_back` has pushed that branch into the
    /// checkout. `git fetch` then refuses `+refs/heads/*:refs/heads/*` with
    /// `refusing to fetch into branch '…' checked out at …` — and `git fetch`
    /// is atomic across refspecs, so batching the remote-tracking mirror into
    /// the same invocation made it fail wholesale for every repository that
    /// already had a workspace. The fix works on the first workspace either
    /// way; only this test tells the two apart.
    #[tokio::test]
    async fn the_remote_tracking_mirror_survives_a_checked_out_branch() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let checkout = fixture.path().join("srv-src").join("repo");
        fs::create_dir_all(&shared_root).unwrap();
        seed_checkout_with_remote(&checkout);
        let repo = repo_record("repo", &checkout);

        let store_handle = store_with_locks(&shared_root).await;
        let store = store_handle.ensure(&repo, "origin/main").await.unwrap();

        // A live workspace: a branch in the store, checked out by a worktree,
        // and present in the checkout too — exactly what `mirror_branch_back`
        // produces.
        git::GitService::new()
            .create_branch(&store, "vk/live", "origin/main")
            .unwrap();
        let worktree = shared_root.join("workspaces").join("live").join("repo");
        fs::create_dir_all(worktree.parent().unwrap()).unwrap();
        cli()
            .worktree_add(&store, &worktree, "vk/live", false)
            .unwrap();
        cli()
            .git(&checkout, ["update-ref", "refs/heads/vk/live", "HEAD"])
            .unwrap();

        // The checkout's origin/main moves on, as it does when git-projects
        // refreshes it.
        fs::write(checkout.join("later.txt"), "later\n").unwrap();
        cli().add_all(&checkout).unwrap();
        cli().commit(&checkout, "upstream moves on").unwrap();
        cli()
            .git(
                &checkout,
                ["update-ref", "refs/remotes/origin/main", "HEAD"],
            )
            .unwrap();
        let moved = cli()
            .resolve_commit(&checkout, "refs/remotes/origin/main")
            .unwrap()
            .unwrap();

        store_handle.ensure(&repo, "origin/main").await.unwrap();

        assert_eq!(
            cli()
                .resolve_commit(&store, "refs/remotes/origin/main")
                .unwrap()
                .unwrap(),
            moved,
            "the remote-tracking mirror must still land while a branch is checked out"
        );
    }

    /// A store that resolves the target branch must not therefore stop
    /// refreshing it. A remote-tracking ref is a copy of something that moves.
    #[tokio::test]
    async fn a_moved_target_branch_is_picked_up_by_the_next_provisioning() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let checkout = fixture.path().join("srv-src").join("repo");
        fs::create_dir_all(&shared_root).unwrap();
        seed_checkout_with_remote(&checkout);
        let repo = repo_record("repo", &checkout);

        let store_handle = store_with_locks(&shared_root).await;
        let store = store_handle.ensure(&repo, "origin/main").await.unwrap();
        let first = cli()
            .resolve_commit(&store, "refs/remotes/origin/main")
            .unwrap()
            .unwrap();

        fs::write(checkout.join("newer.txt"), "newer\n").unwrap();
        cli().add_all(&checkout).unwrap();
        cli().commit(&checkout, "upstream advances").unwrap();
        cli()
            .git(
                &checkout,
                ["update-ref", "refs/remotes/origin/main", "HEAD"],
            )
            .unwrap();

        store_handle.ensure(&repo, "origin/main").await.unwrap();

        let second = cli()
            .resolve_commit(&store, "refs/remotes/origin/main")
            .unwrap()
            .unwrap();
        assert_ne!(
            second, first,
            "the second workspace would otherwise branch from a frozen commit \
             while the picker showed the current one"
        );
        assert_eq!(
            second,
            cli()
                .resolve_commit(&checkout, "refs/remotes/origin/main")
                .unwrap()
                .unwrap()
        );
    }

    /// `adopt` rewrites a live worktree's HEAD to `refs/heads/{branch}`, so a
    /// same-named remote-tracking ref must not satisfy its guard — the reset
    /// that follows would clear the index rather than rebuild it.
    #[tokio::test]
    async fn adopt_refuses_a_branch_that_is_only_a_remote_tracking_ref() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let node_local = fixture.path().join("srv-src");
        fs::create_dir_all(&shared_root).unwrap();
        fs::create_dir_all(&node_local).unwrap();

        let repo_path = node_local.join("repo");
        seed_repo(&repo_path);
        let repo = repo_record("repo", &repo_path);
        let store = shared_root.join("repositories").join(repo.id.to_string());
        fs::create_dir_all(store.parent().unwrap()).unwrap();
        cli().clone_bare(&repo_path, &store).unwrap();
        // The store knows `origin/wip` only as a remote-tracking ref.
        cli()
            .git(&store, ["update-ref", "refs/remotes/origin/wip", "main"])
            .unwrap();

        let workspace = shared_root.join("workspaces").join("ws-1");
        let worktree = workspace.join("repo");
        fs::create_dir_all(&workspace).unwrap();
        git::GitService::new()
            .create_branch(&repo_path, "origin/wip", "main")
            .unwrap();
        cli()
            .worktree_add(&repo_path, &worktree, "origin/wip", false)
            .unwrap();
        fs::write(worktree.join("agent-work.txt"), "irreplaceable\n").unwrap();
        fs::rename(&repo_path, node_local.join("moved-away")).unwrap();

        let outcome = store_for(&shared_root)
            .adopt(&repo, &worktree, "origin/wip")
            .await
            .unwrap();

        match outcome {
            AdoptOutcome::Skipped { reason } => assert!(reason.contains("not present"), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(
            fs::read_to_string(worktree.join("agent-work.txt")).unwrap(),
            "irreplaceable\n",
            "a refusal must leave the working tree untouched"
        );
    }

    /// The store must answer "does this name a branch here?" the same way
    /// `GitService::find_branch` does — local first, then remote-tracking — or
    /// it cannot serve the branches the rest of the product already accepted.
    #[tokio::test]
    async fn branch_resolution_prefers_a_local_branch_over_a_remote_one() {
        let fixture = TempDir::new().unwrap();
        let checkout = fixture.path().join("repo");
        seed_repo(&checkout);
        fs::write(checkout.join("other.txt"), "other\n").unwrap();
        cli().add_all(&checkout).unwrap();
        cli().commit(&checkout, "second").unwrap();
        // `shared` exists as both a local and a remote-tracking branch, at
        // different commits.
        cli()
            .git(&checkout, ["update-ref", "refs/remotes/shared", "HEAD"])
            .unwrap();
        cli()
            .git(&checkout, ["update-ref", "refs/heads/shared", "HEAD~1"])
            .unwrap();

        assert_eq!(
            SharedRepositoryStore::resolved_branch_ref(&cli(), &checkout, "shared").unwrap(),
            Some("refs/heads/shared".to_string())
        );
        assert_eq!(
            SharedRepositoryStore::resolved_branch_ref(&cli(), &checkout, "nope").unwrap(),
            None
        );
    }

    /// Pins the duplicated rule to the one it copies. This resolver cannot call
    /// `GitService::find_branch` — that answers "does the ref exist", while a
    /// store must prove the commit object is present — so the agreement is
    /// asserted instead of assumed.
    #[test]
    fn branch_resolution_agrees_with_git_services_branch_lookup() {
        let fixture = TempDir::new().unwrap();
        let checkout = fixture.path().join("repo");
        seed_repo(&checkout);
        cli()
            .git(
                &checkout,
                ["update-ref", "refs/remotes/origin/main", "HEAD"],
            )
            .unwrap();

        let git = git::GitService::new();
        for branch in ["main", "origin/main", "absent", "origin/absent"] {
            assert_eq!(
                SharedRepositoryStore::resolved_branch_ref(&cli(), &checkout, branch)
                    .unwrap()
                    .is_some(),
                git.check_branch_exists(&checkout, branch).unwrap(),
                "the two resolvers disagree about '{branch}'"
            );
        }
    }

    /// Spelling out the two namespaces is not redundant with git's own revision
    /// precedence. That precedence also accepts `refs/tags/<name>`, so
    /// delegating to it would let a tag stand in for a branch — which
    /// `GitService::find_branch` never does, and which would put a workspace on
    /// a base that is not a branch at all.
    #[test]
    fn a_tag_is_not_a_branch() {
        let fixture = TempDir::new().unwrap();
        let checkout = fixture.path().join("repo");
        seed_repo(&checkout);
        cli().git(&checkout, ["tag", "only-a-tag", "HEAD"]).unwrap();

        assert!(
            cli().commit_exists(&checkout, "only-a-tag").unwrap(),
            "precondition: git's bare-name precedence does resolve the tag"
        );
        assert_eq!(
            SharedRepositoryStore::resolved_branch_ref(&cli(), &checkout, "only-a-tag").unwrap(),
            None,
            "a tag must not satisfy a target branch"
        );
        assert!(
            !git::GitService::new()
                .check_branch_exists(&checkout, "only-a-tag")
                .unwrap(),
            "and the rule being matched agrees"
        );
    }

    #[test]
    fn a_git_failure_is_reduced_to_one_bounded_line() {
        assert_eq!(
            SharedRepositoryStore::summarize(
                "\n  fatal: could not read Username for 'https://github.com'\n--- stderr\nremote: lots\n"
            ),
            "fatal: could not read Username for 'https://github.com'"
        );
        assert_eq!(SharedRepositoryStore::summarize("   \n\n"), "no output");
        let long = "x".repeat(500);
        let summarized = SharedRepositoryStore::summarize(&long);
        assert!(summarized.ends_with('\u{2026}'));
        assert_eq!(summarized.chars().count(), 201);
    }

    #[test]
    fn fallback_refspec_targets_the_remote_tracking_namespace() {
        // A remote-prefixed target names a branch upstream knows by another
        // name, and belongs where the resolver will look for it.
        assert_eq!(
            SharedRepositoryStore::fallback_refspec("origin", "origin/main"),
            "+refs/heads/main:refs/remotes/origin/main"
        );
        assert_eq!(
            SharedRepositoryStore::fallback_refspec("origin", "origin/release/1.x"),
            "+refs/heads/release/1.x:refs/remotes/origin/release/1.x"
        );
        // A plain local name keeps the original local-to-local form.
        assert_eq!(
            SharedRepositoryStore::fallback_refspec("origin", "main"),
            "+refs/heads/main:refs/heads/main"
        );
        // Prefixed with a *different* remote's name: nothing says this remote
        // holds it under another name, so do not guess.
        assert_eq!(
            SharedRepositoryStore::fallback_refspec("upstream", "origin/main"),
            "+refs/heads/origin/main:refs/heads/origin/main"
        );
    }

    /// A branch that exists nowhere must still fail — and say which repository
    /// and which branch, because that failure is shown to the user.
    #[tokio::test]
    async fn ensure_reports_which_repository_and_branch_it_could_not_serve() {
        let fixture = TempDir::new().unwrap();
        let shared_root = fixture.path().join("shared");
        let checkout = fixture.path().join("srv-src").join("repo");
        fs::create_dir_all(&shared_root).unwrap();
        seed_checkout_with_remote(&checkout);
        let repo = repo_record("repo", &checkout);

        let error = store_with_locks(&shared_root)
            .await
            .ensure(&repo, "origin/no-such-branch")
            .await
            .expect_err("a branch that exists nowhere cannot be served");

        match &error {
            WorkspaceError::SharedStore {
                repo_name, branch, ..
            } => {
                assert_eq!(repo_name, "repo");
                assert_eq!(branch, "origin/no-such-branch");
            }
            other => panic!("expected SharedStore, got {other:?}"),
        }
        let rendered = error.to_string();
        assert!(rendered.contains("repo"), "{rendered}");
        assert!(rendered.contains("origin/no-such-branch"), "{rendered}");
    }

    fn unreachable_locks() -> RepositoryAdminLockManager {
        // A manager whose pool is never used: these tests exercise `adopt`,
        // which does no locking of its own.
        let pool = sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap();
        RepositoryAdminLockManager::new(pool, std::time::Duration::from_secs(30)).unwrap()
    }
}
