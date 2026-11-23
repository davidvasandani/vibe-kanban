//! Per-worktree commit identity.
//!
//! An organization can pin the git identity its workspaces commit as. Linked
//! worktrees share their repository's `config`, which for a coordinator-local
//! workspace is the operator's own checkout, so the identity is written to the
//! worktree's own `config.worktree` (enabled by `extensions.worktreeConfig`).
//! The same file points `core.hooksPath` at a hooks directory inside the
//! worktree's git dir, whose `pre-commit` rejects any author or committer
//! email that is not the organization's or on its allowlist, and whose other
//! hooks forward to the repository's own.

use std::path::{Path, PathBuf};

use crate::{GitCli, GitServiceError};

/// The identity to enforce on one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCommitIdentity {
    pub name: String,
    pub email: String,
    /// Further author/committer emails the pre-commit check accepts (agent
    /// and bot identities).
    pub allowed_emails: Vec<String>,
}

/// Directory, inside a worktree's git dir, that holds the installed hooks.
const HOOKS_DIR_NAME: &str = "vk-hooks";
const COMMIT_EMAIL_KEY: &str = "vibe-kanban.commitEmail";
const ALLOWED_EMAIL_KEY: &str = "vibe-kanban.allowedCommitEmail";

/// Every client-side hook git looks up in `core.hooksPath`. Pointing that at
/// our directory hides the repository's own hooks, so each one forwards.
const CLIENT_HOOKS: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "pre-push",
    "pre-auto-gc",
    "post-rewrite",
    "sendemail-validate",
    "push-to-checkout",
    "reference-transaction",
    "post-index-change",
    "p4-changelist",
    "p4-prepare-changelist",
    "p4-post-changelist",
    "p4-pre-submit",
];

/// Installed under every name in [`CLIENT_HOOKS`]. `pre-commit` and
/// `pre-merge-commit` check the identity first; every hook then runs the
/// repository's own hook of the same name, if there is one.
const HOOK_SCRIPT: &str = r#"#!/bin/sh
# Installed by Vibe Kanban: enforces this organization's commit identity on
# this worktree, then runs the repository's own hook of the same name.
# Globs are off so an allowlisted address such as renovate[bot]@... is
# compared literally.
set -f
hook=$(basename "$0")

case "$hook" in
pre-commit | pre-merge-commit)
	expected=$(git config --worktree --get vibe-kanban.commitEmail)
	if [ -n "$expected" ]; then
		allowed=$(git config --worktree --get-all vibe-kanban.allowedCommitEmail)
		for role in AUTHOR COMMITTER; do
			ident=$(git var "GIT_${role}_IDENT") || exit 1
			email=$(printf '%s\n' "$ident" | sed -n 's/^[^<]*<\([^>]*\)>.*$/\1/p' | tr '[:upper:]' '[:lower:]')
			ok=
			for candidate in $expected $allowed; do
				if [ "$email" = "$candidate" ]; then
					ok=1
					break
				fi
			done
			if [ -z "$ok" ]; then
				role_name=$(printf '%s' "$role" | tr '[:upper:]' '[:lower:]')
				echo "vibe-kanban: commit rejected: $role_name email <$email> is not this organization's commit email <$expected> or an allowed agent address." >&2
				echo "vibe-kanban: fix it with: git config --worktree user.email $expected" >&2
				exit 1
			fi
		done
	fi
	;;
esac

orig=$(git config --local --type=path --get core.hooksPath 2>/dev/null ||
	git config --global --type=path --get core.hooksPath 2>/dev/null ||
	git config --system --type=path --get core.hooksPath 2>/dev/null)
if [ -z "$orig" ]; then
	orig="$(git rev-parse --git-common-dir)/hooks" || exit 1
fi
target="$orig/$hook"
if [ -x "$target" ]; then
	self_dir=$(cd "$(dirname "$0")" && pwd -P)
	target_dir=$(cd "$(dirname "$target")" && pwd -P)
	if [ "$self_dir" != "$target_dir" ]; then
		exec "$target" "$@"
	fi
fi
exit 0
"#;

struct WorktreeDirs {
    git_dir: PathBuf,
    common_dir: PathBuf,
}

fn worktree_dirs(git: &GitCli, worktree: &Path) -> Result<WorktreeDirs, GitServiceError> {
    let out = git.git(
        worktree,
        [
            "rev-parse",
            "--path-format=absolute",
            "--git-dir",
            "--git-common-dir",
        ],
    )?;
    let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty());
    match (lines.next(), lines.next()) {
        (Some(git_dir), Some(common_dir)) => Ok(WorktreeDirs {
            git_dir: PathBuf::from(git_dir),
            common_dir: PathBuf::from(common_dir),
        }),
        _ => Err(GitServiceError::InvalidRepository(format!(
            "could not resolve git dirs for {}",
            worktree.display()
        ))),
    }
}

fn config_get(git: &GitCli, cwd: &Path, file: &Path, key: &str) -> Option<String> {
    let file = file.to_string_lossy();
    git.git(cwd, ["config", "--file", file.as_ref(), "--get", key])
        .ok()
        .map(|v| v.trim().to_owned())
}

fn config_set(
    git: &GitCli,
    cwd: &Path,
    file: &Path,
    key: &str,
    value: &str,
) -> Result<(), GitServiceError> {
    let file = file.to_string_lossy();
    git.git(cwd, ["config", "--file", file.as_ref(), key, value])?;
    Ok(())
}

/// Removing a key that is not set is not an error.
fn config_unset_all(git: &GitCli, cwd: &Path, file: &Path, key: &str) {
    let file = file.to_string_lossy();
    let _ = git.git(cwd, ["config", "--file", file.as_ref(), "--unset-all", key]);
}

/// Turn on `extensions.worktreeConfig` for the repository, first moving the
/// two settings that must then live in the main worktree's own file: with the
/// extension on, a `core.bare = true` left in the shared config is read by
/// every linked worktree, which then refuses to run anything ("this operation
/// must be run in a work tree"). The cluster's shared stores are bare.
fn enable_worktree_config(git: &GitCli, dirs: &WorktreeDirs) -> Result<(), GitServiceError> {
    let cwd = &dirs.common_dir;
    let common_config = dirs.common_dir.join("config");
    if config_get(git, cwd, &common_config, "extensions.worktreeConfig").as_deref() == Some("true")
    {
        return Ok(());
    }
    let main_worktree_config = dirs.common_dir.join("config.worktree");
    // Written before being removed from the shared file, and the extension is
    // enabled last, so no reader ever sees a linked worktree as bare.
    if config_get(git, cwd, &common_config, "core.bare").as_deref() == Some("true") {
        config_set(git, cwd, &main_worktree_config, "core.bare", "true")?;
        config_unset_all(git, cwd, &common_config, "core.bare");
    }
    if let Some(core_worktree) = config_get(git, cwd, &common_config, "core.worktree") {
        config_set(
            git,
            cwd,
            &main_worktree_config,
            "core.worktree",
            &core_worktree,
        )?;
        config_unset_all(git, cwd, &common_config, "core.worktree");
    }
    config_set(
        git,
        cwd,
        &common_config,
        "extensions.worktreeConfig",
        "true",
    )
}

fn install_hooks(hooks_dir: &Path) -> Result<(), GitServiceError> {
    std::fs::create_dir_all(hooks_dir)?;
    for hook in CLIENT_HOOKS {
        let path = hooks_dir.join(hook);
        std::fs::write(&path, HOOK_SCRIPT)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

/// Apply `identity` to the linked worktree at `worktree`, or remove a
/// previously applied one when `identity` is `None`. Idempotent. A main
/// worktree (the user's own checkout) is left untouched.
pub(crate) fn apply(
    worktree: &Path,
    identity: Option<&WorktreeCommitIdentity>,
) -> Result<(), GitServiceError> {
    let git = GitCli::new();
    let dirs = worktree_dirs(&git, worktree)?;
    if dirs.git_dir == dirs.common_dir {
        tracing::debug!(
            worktree = %worktree.display(),
            "not a linked worktree; leaving its commit identity alone"
        );
        return Ok(());
    }

    let worktree_config = dirs.git_dir.join("config.worktree");
    let hooks_dir = dirs.git_dir.join(HOOKS_DIR_NAME);

    let Some(identity) = identity else {
        if !worktree_config.exists() {
            return Ok(());
        }
        for key in [
            "user.name",
            "user.email",
            COMMIT_EMAIL_KEY,
            ALLOWED_EMAIL_KEY,
        ] {
            config_unset_all(&git, worktree, &worktree_config, key);
        }
        if config_get(&git, worktree, &worktree_config, "core.hooksPath")
            .is_some_and(|path| Path::new(&path) == hooks_dir)
        {
            config_unset_all(&git, worktree, &worktree_config, "core.hooksPath");
        }
        if hooks_dir.exists() {
            std::fs::remove_dir_all(&hooks_dir)?;
        }
        return Ok(());
    };

    enable_worktree_config(&git, &dirs)?;
    // Hooks first: core.hooksPath must never name a directory that is not
    // fully written yet.
    install_hooks(&hooks_dir)?;
    config_set(
        &git,
        worktree,
        &worktree_config,
        "user.name",
        &identity.name,
    )?;
    config_set(
        &git,
        worktree,
        &worktree_config,
        "user.email",
        &identity.email,
    )?;
    config_set(
        &git,
        worktree,
        &worktree_config,
        COMMIT_EMAIL_KEY,
        &identity.email,
    )?;
    config_unset_all(&git, worktree, &worktree_config, ALLOWED_EMAIL_KEY);
    for email in &identity.allowed_emails {
        let file = worktree_config.to_string_lossy();
        git.git(
            worktree,
            [
                "config",
                "--file",
                file.as_ref(),
                "--add",
                ALLOWED_EMAIL_KEY,
                email,
            ],
        )?;
    }
    config_set(
        &git,
        worktree,
        &worktree_config,
        "core.hooksPath",
        &hooks_dir.to_string_lossy(),
    )
}
