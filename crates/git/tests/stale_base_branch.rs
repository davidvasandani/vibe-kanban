//! VAS-1069: a target branch that is checked out in a worktree, and therefore
//! cannot be fast-forwarded, must not make new workspaces start stale.

use git::{GitService, STALE_BASE_WARN_THRESHOLD};
use git2::{Oid, Repository, Signature};
use tempfile::TempDir;

fn commit_on(repo: &Repository, parent: Option<Oid>, message: &str) -> Oid {
    let sig = Signature::now("Test User", "test@example.com").unwrap();
    let tree = repo
        .find_tree(repo.treebuilder(None).unwrap().write().unwrap())
        .unwrap();
    let parent = parent.map(|id| repo.find_commit(id).unwrap());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    repo.commit(None, &sig, &sig, message, &tree, &parents)
        .unwrap()
}

/// A repository whose `main` is checked out and sits at the first commit,
/// while `origin/main` has since moved `ahead` commits further on.
fn repo_with_stale_checked_out_main(ahead: usize) -> (TempDir, Oid, Oid) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let stale = commit_on(&repo, None, "stale base");
    repo.reference("refs/heads/main", stale, true, "main")
        .unwrap();
    repo.set_head("refs/heads/main").unwrap();
    let mut tip = stale;
    for i in 0..ahead {
        tip = commit_on(&repo, Some(tip), &format!("upstream {i}"));
    }
    repo.reference("refs/remotes/origin/main", tip, true, "fetch")
        .unwrap();
    (dir, stale, tip)
}

fn branch_tip(dir: &TempDir, name: &str) -> Oid {
    let repo = Repository::open(dir.path()).unwrap();
    repo.find_branch(name, git2::BranchType::Local)
        .unwrap()
        .get()
        .peel_to_commit()
        .unwrap()
        .id()
}

#[test]
fn a_checked_out_stale_target_branch_does_not_make_a_new_workspace_start_stale() {
    let (dir, stale, tip) = repo_with_stale_checked_out_main(3);
    let git = GitService::new();

    git.create_branch(dir.path(), "vk/new-workspace", "main")
        .unwrap();

    assert_eq!(branch_tip(&dir, "vk/new-workspace"), tip);
    // The checked-out ref itself is never moved out from under its worktree.
    assert_eq!(branch_tip(&dir, "main"), stale);
}

#[test]
fn a_local_target_branch_that_is_current_is_used_as_is() {
    let (dir, stale, _) = repo_with_stale_checked_out_main(0);
    GitService::new()
        .create_branch(dir.path(), "vk/new-workspace", "main")
        .unwrap();
    assert_eq!(branch_tip(&dir, "vk/new-workspace"), stale);
}

#[test]
fn a_local_target_branch_ahead_of_origin_is_not_rewound() {
    let (dir, _, tip) = repo_with_stale_checked_out_main(2);
    let repo = Repository::open(dir.path()).unwrap();
    let local_only = commit_on(&repo, Some(tip), "unpushed local work");
    repo.reference("refs/heads/main", local_only, true, "local")
        .unwrap();
    GitService::new()
        .create_branch(dir.path(), "vk/new-workspace", "main")
        .unwrap();
    assert_eq!(branch_tip(&dir, "vk/new-workspace"), local_only);
}

#[test]
fn a_diverged_local_target_branch_is_not_replaced_by_origin() {
    let (dir, stale, _) = repo_with_stale_checked_out_main(2);
    let repo = Repository::open(dir.path()).unwrap();
    let diverged = commit_on(&repo, Some(stale), "local only, not on origin");
    repo.reference("refs/heads/main", diverged, true, "local")
        .unwrap();
    GitService::new()
        .create_branch(dir.path(), "vk/new-workspace", "main")
        .unwrap();
    assert_eq!(branch_tip(&dir, "vk/new-workspace"), diverged);
}

#[test]
fn an_explicit_remote_base_is_used_exactly() {
    let (dir, _, tip) = repo_with_stale_checked_out_main(3);
    GitService::new()
        .create_branch(dir.path(), "vk/new-workspace", "origin/main")
        .unwrap();
    assert_eq!(branch_tip(&dir, "vk/new-workspace"), tip);
}

#[test]
fn base_freshness_reports_the_base_and_how_far_behind_origin_it_is() {
    let (dir, stale, _) = repo_with_stale_checked_out_main(STALE_BASE_WARN_THRESHOLD + 1);
    let git = GitService::new();

    let freshness = git.base_freshness(dir.path(), "main").unwrap();
    assert_eq!(freshness.base_commit, stale.to_string());
    assert_eq!(
        freshness.commits_behind,
        Some(STALE_BASE_WARN_THRESHOLD + 1)
    );
    assert!(freshness.is_stale());
    let line = freshness.describe("homelab");
    assert!(line.contains("WARNING"), "{line}");
    assert!(line.contains(&stale.to_string()[..8]), "{line}");

    // `origin/main` as the target name compares against the same upstream.
    assert_eq!(
        git.base_freshness(dir.path(), "origin/main").unwrap(),
        freshness
    );
}

#[test]
fn a_base_within_the_threshold_is_not_flagged() {
    let (dir, _, _) = repo_with_stale_checked_out_main(2);
    let freshness = GitService::new()
        .base_freshness(dir.path(), "main")
        .unwrap();
    assert_eq!(freshness.commits_behind, Some(2));
    assert!(!freshness.is_stale());
    assert!(!freshness.describe("homelab").contains("WARNING"));
}

#[test]
fn a_repository_without_an_origin_branch_has_nothing_to_be_behind() {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();
    let c = commit_on(&repo, None, "only commit");
    repo.reference("refs/heads/main", c, true, "main").unwrap();
    repo.set_head("refs/heads/main").unwrap();
    let freshness = GitService::new()
        .base_freshness(dir.path(), "main")
        .unwrap();
    assert_eq!(freshness.commits_behind, None);
    assert!(!freshness.is_stale());
}
