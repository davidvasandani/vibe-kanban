//! `gh` invocations authenticate with the target owner's org token, not the
//! server's ambient credentials. Runs a fake `gh` first on PATH; one test
//! function, because it changes this process's environment.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use git_host::github::GhCli;
use utils::github_credentials::{GitHubCredentials, GitHubToken, OwnerCredential};

const ORG_TOKEN: &str = "synthetic-sweetgreen-token";

#[test]
fn gh_uses_org_tokens_by_target_owner_and_attributes_failures() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let invoked = root.path().join("invoked");
    // `repo view` reports the token it saw as the owner login; a check-runs
    // read fails the way a fine-grained PAT without Checks permission does.
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> '{invoked}'
case "$*" in
  *check-runs*) echo "gh: Resource not accessible by personal access token (HTTP 403)" >&2; exit 1 ;;
esac
printf '{{"owner":{{"login":"%s"}},"name":"r","url":"https://github.com/o/r"}}' "${{GH_TOKEN:-${{GITHUB_TOKEN:-none}}}}"
"#,
        invoked = invoked.display()
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    // SAFETY: single test in this binary; no other thread reads the env.
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("GH_TOKEN", "ambient-gh");
        std::env::set_var("GITHUB_TOKEN", "ambient-github");
    }

    let mut credentials = GitHubCredentials::default();
    credentials.insert(
        "Sweetgreen",
        OwnerCredential::OrgToken(GitHubToken::new(ORG_TOKEN)),
    );
    credentials.insert(
        "broken",
        OwnerCredential::Unavailable("1Password lookup failed".into()),
    );
    let cli = GhCli::with_credentials(credentials);
    let work = root.path();

    // Configured owner (any case, any URL form): the org token wins.
    for url in [
        "https://github.com/sweetgreen/platform-ops",
        "git@github.com:SWEETGREEN/platform-ops.git",
    ] {
        let info = cli.get_repo_info(url, work).unwrap();
        assert_eq!(info.owner, ORG_TOKEN, "{url}");
    }
    // Unconfigured owner: the server's existing credential (fallback).
    let info = cli
        .get_repo_info("https://github.com/someone/x", work)
        .unwrap();
    assert_eq!(info.owner, "ambient-gh");
    // Plain GhCli (no credentials) behaves as before.
    let info = GhCli::new()
        .get_repo_info("https://github.com/sweetgreen/platform-ops", work)
        .unwrap();
    assert_eq!(info.owner, "ambient-gh");

    // A 403 names the owner and source, and stays a permission error so check
    // coverage still reads it as "forbidden".
    let mut target = cli
        .get_repo_info("https://github.com/sweetgreen/platform-ops", work)
        .unwrap();
    target.owner = "sweetgreen".into();
    target.repo_name = "platform-ops".into();
    let error = cli
        .list_check_runs(&target, "abc123", work)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("the sweetgreen org token")
            && error.contains("sweetgreen/platform-ops")
            && error.contains("HTTP 403"),
        "{error}"
    );
    target.owner = "someone".into();
    let error = cli
        .list_check_runs(&target, "abc123", work)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("no org token for owner someone; used the server's fallback credential"),
        "{error}"
    );

    // An unreadable configured token fails closed without running gh.
    let before = std::fs::read_to_string(&invoked).unwrap().lines().count();
    let error = cli
        .get_repo_info("https://github.com/broken/x", work)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("broken org token is configured but unavailable"),
        "{error}"
    );
    let after = std::fs::read_to_string(&invoked).unwrap().lines().count();
    assert_eq!(before, after, "gh must not run for an unavailable token");

    // `gh pr checkout` is refused when an inherited rewrite would send its
    // fetch past the org token (here: an exact-URL rule, which outranks ours).
    let checkout = root.path().join("checkout");
    let init = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    };
    init(&["init", "-q", checkout.to_str().unwrap()]);
    let checkout_str = checkout.to_str().unwrap();
    init(&[
        "-C",
        checkout_str,
        "remote",
        "add",
        "origin",
        "https://github.com/sweetgreen/platform-ops.git",
    ]);
    let before = std::fs::read_to_string(&invoked).unwrap().lines().count();
    cli.pr_checkout(&checkout, "sweetgreen", "platform-ops", 7)
        .expect("no rewrite: checkout runs");
    let after_ok = std::fs::read_to_string(&invoked).unwrap().lines().count();
    assert_eq!(after_ok, before + 1);
    init(&[
        "-C",
        checkout_str,
        "config",
        "url.git@github.com:sweetgreen/platform-ops.git.insteadOf",
        "https://github.com/sweetgreen/platform-ops.git",
    ]);
    let error = cli
        .pr_checkout(&checkout, "sweetgreen", "platform-ops", 7)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("sweetgreen org token cannot be used"),
        "{error}"
    );
    let after = std::fs::read_to_string(&invoked).unwrap().lines().count();
    assert_eq!(
        after, after_ok,
        "gh must not run when the fetch would bypass the token"
    );
}
