//! Per-worktree commit identity. These run the real `git` binary: the point is
//! what an agent's own `git commit` does, hooks included, and GitService runs
//! every command with hooks disabled.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use git::{GitService, WorktreeCommitIdentity};
use git2::Repository;
use tempfile::TempDir;

/// Run git isolated from the machine's global and system config, so a
/// developer's own identity or hooksPath cannot leak into the result.
fn git_raw(cwd: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .current_dir(cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .args(args)
        .output()
        .expect("git runs")
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = git_raw(cwd, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn identity() -> WorktreeCommitIdentity {
    WorktreeCommitIdentity {
        name: "Org Owner".to_owned(),
        email: "owner@org.example".to_owned(),
        allowed_emails: vec![
            "noreply@anthropic.com".to_owned(),
            "29139614+renovate[bot]@users.noreply.github.com".to_owned(),
        ],
    }
}

struct Fixture {
    _tmp: TempDir,
    main: PathBuf,
    worktree: PathBuf,
}

/// A normal checkout, with a user identity of its own, and one linked worktree.
fn checkout_with_worktree() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let main = tmp.path().join("main");
    fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    git(&main, &["config", "user.name", "Operator"]);
    git(&main, &["config", "user.email", "operator@work.example"]);
    git(&main, &["commit", "-q", "--allow-empty", "-m", "init"]);
    let worktree = tmp.path().join("wt");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "task",
            worktree.to_str().unwrap(),
        ],
    );
    Fixture {
        _tmp: tmp,
        main,
        worktree,
    }
}

fn commit(worktree: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["commit", "-q", "--allow-empty", "-m", "change"];
    args.extend_from_slice(extra);
    git_raw(worktree, &args)
}

#[test]
fn worktree_commits_as_the_org_and_the_checkout_keeps_its_identity() {
    let f = checkout_with_worktree();
    GitService::new()
        .apply_worktree_commit_identity(&f.worktree, Some(&identity()))
        .unwrap();

    let out = commit(&f.worktree, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        git(&f.worktree, &["log", "-1", "--format=%an <%ae>|%cn <%ce>"]),
        "Org Owner <owner@org.example>|Org Owner <owner@org.example>"
    );

    // The operator's checkout and the shared config are untouched.
    assert_eq!(
        git(&f.main, &["config", "user.email"]),
        "operator@work.example"
    );
    assert_eq!(
        git(&f.main, &["config", "--local", "user.email"]),
        "operator@work.example"
    );

    // libgit2 (GitService's own commits) resolves the same identity.
    let sig = Repository::open(&f.worktree).unwrap().signature().unwrap();
    assert_eq!(sig.email(), Some("owner@org.example"));
}

#[test]
fn pre_commit_rejects_other_author_and_committer_emails() {
    let f = checkout_with_worktree();
    GitService::new()
        .apply_worktree_commit_identity(&f.worktree, Some(&identity()))
        .unwrap();

    let bad_author = commit(&f.worktree, &["--author", "Someone <someone@work.example>"]);
    assert!(!bad_author.status.success());
    let stderr = String::from_utf8_lossy(&bad_author.stderr);
    assert!(
        stderr.contains("author email <someone@work.example>"),
        "{stderr}"
    );

    let bad_committer = git_raw(
        &f.worktree,
        &[
            "-c",
            "user.email=davidvasandani@laptop.local",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ],
    );
    assert!(!bad_committer.status.success());

    // Allowlisted agent identities pass, compared case-insensitively and
    // literally (a file matching the `[bot]` glob must not change anything).
    fs::write(
        f.worktree
            .join("29139614+renovatet@users.noreply.github.com"),
        "",
    )
    .unwrap();
    for author in [
        "Claude <NoReply@Anthropic.com>",
        "renovate[bot] <29139614+renovate[bot]@users.noreply.github.com>",
    ] {
        let out = commit(&f.worktree, &["--author", author]);
        assert!(
            out.status.success(),
            "{author}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn repository_hooks_still_run() {
    let f = checkout_with_worktree();
    let hooks = f.main.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    let marker = f.main.join("hook-ran");
    let script = format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.to_string_lossy());
    fs::write(hooks.join("commit-msg"), script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hooks.join("commit-msg"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    GitService::new()
        .apply_worktree_commit_identity(&f.worktree, Some(&identity()))
        .unwrap();

    let out = commit(&f.worktree, &[]);
    assert!(
        !out.status.success(),
        "the repository's failing hook must still apply"
    );
    assert!(marker.exists());
}

#[test]
fn removing_the_identity_restores_the_worktree() {
    let f = checkout_with_worktree();
    let service = GitService::new();
    service
        .apply_worktree_commit_identity(&f.worktree, Some(&identity()))
        .unwrap();
    // Idempotent.
    service
        .apply_worktree_commit_identity(&f.worktree, Some(&identity()))
        .unwrap();
    service
        .apply_worktree_commit_identity(&f.worktree, None)
        .unwrap();

    assert_eq!(
        git(&f.worktree, &["config", "user.email"]),
        "operator@work.example"
    );
    assert!(
        git_raw(&f.worktree, &["config", "core.hooksPath"])
            .stdout
            .is_empty()
    );
    let out = commit(&f.worktree, &["--author", "Someone <someone@work.example>"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn main_checkout_is_never_modified() {
    let f = checkout_with_worktree();
    GitService::new()
        .apply_worktree_commit_identity(&f.main, Some(&identity()))
        .unwrap();
    assert_eq!(
        git(&f.main, &["config", "user.email"]),
        "operator@work.example"
    );
    assert!(
        git_raw(&f.main, &["config", "extensions.worktreeConfig"])
            .stdout
            .is_empty()
    );
}

/// The cluster's shared stores are bare repositories. Enabling
/// `extensions.worktreeConfig` with `core.bare = true` still in the shared
/// config would make every linked worktree think it is bare.
#[test]
fn bare_store_worktrees_keep_working() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src");
    fs::create_dir(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    git(
        &src,
        &[
            "-c",
            "user.name=a",
            "-c",
            "user.email=a@b.c",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
    );
    let store = tmp.path().join("store.git");
    git(
        tmp.path(),
        &[
            "clone",
            "-q",
            "--bare",
            src.to_str().unwrap(),
            store.to_str().unwrap(),
        ],
    );
    let wt1 = tmp.path().join("wt1");
    let wt2 = tmp.path().join("wt2");
    git(
        &store,
        &["worktree", "add", "-q", "-b", "one", wt1.to_str().unwrap()],
    );
    git(
        &store,
        &["worktree", "add", "-q", "-b", "two", wt2.to_str().unwrap()],
    );

    GitService::new()
        .apply_worktree_commit_identity(&wt1, Some(&identity()))
        .unwrap();

    let out = commit(&wt1, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // A sibling worktree without an identity still works, and inherits none.
    git(&wt2, &["status", "--short"]);
    assert!(git_raw(&wt2, &["config", "user.email"]).stdout.is_empty());
    // The store stays bare for both git and libgit2, and new worktrees work.
    assert_eq!(git(&store, &["rev-parse", "--is-bare-repository"]), "true");
    assert!(Repository::open(&store).unwrap().is_bare());
    assert!(!Repository::open(&wt1).unwrap().is_bare());
    let wt3 = tmp.path().join("wt3");
    git(
        &store,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "three",
            wt3.to_str().unwrap(),
        ],
    );
    git(&wt3, &["status", "--short"]);
}
