//! Per-owner GitHub token routing for workspace processes.
//!
//! The coordinator resolves Settings → Repositories → GitHub organization
//! tokens into [`OWNERS_ENV`] plus one [`token_env_name`] variable per owner.
//! The host that spawns a workspace child (coordinator or cluster worker) then
//! calls [`apply_github_routing`] immediately before spawn, which:
//!
//! - prepends a host-local `gh` shim directory to `PATH`, so each `gh`
//!   invocation gets the PAT of the owner it targets; and
//! - appends owner-scoped Git credential helpers through `GIT_CONFIG_*`, so
//!   HTTPS Git operations under `https://github.com/<owner>/` use that PAT.
//!
//! With no owners configured this is a no-op. Values are read by name at
//! runtime; no token is written to disk or into Git configuration text.

use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};

use crate::{assets::github_auth_bin_dir, shell::merge_paths};

// Names deliberately avoid KEY/SECRET/TOKEN: some agent CLIs (Codex's
// default shell environment policy) drop variables matching those words
// from the commands they run, which would silently disable routing there.
/// Comma-separated list of configured owners, as entered.
pub const OWNERS_ENV: &str = "VK_GITHUB_PAT_OWNERS";
const TOKEN_ENV_PREFIX: &str = "VK_GITHUB_PAT_";
const SHIM_TEMPLATE: &str = include_str!("github_auth/gh-shim.sh");
const SHIM_DIR_PLACEHOLDER: &str = "__VK_SHIM_DIR__";

/// Whether `owner` is a valid GitHub login: 1–39 ASCII letters, digits or
/// hyphens, not starting or ending with a hyphen.
pub fn is_valid_owner(owner: &str) -> bool {
    (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
}

/// Environment variable carrying `owner`'s resolved token. GitHub logins
/// cannot contain `_`, so mapping `-` to `_` is injective (case aside, which
/// GitHub also ignores).
pub fn token_env_name(owner: &str) -> String {
    format!(
        "{TOKEN_ENV_PREFIX}{}",
        owner.to_ascii_uppercase().replace('-', "_")
    )
}

/// Owners named by an [`OWNERS_ENV`] value, skipping invalid entries.
pub fn parse_owners(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|owner| is_valid_owner(owner))
        .collect()
}

/// A process environment map a spawn site is about to apply.
pub trait EnvMap {
    fn env_get(&self, key: &str) -> Option<&str>;
    fn env_set(&mut self, key: String, value: String);
}

impl EnvMap for HashMap<String, String> {
    fn env_get(&self, key: &str) -> Option<&str> {
        self.get(key).map(String::as_str)
    }
    fn env_set(&mut self, key: String, value: String) {
        self.insert(key, value);
    }
}

impl EnvMap for BTreeMap<String, String> {
    fn env_get(&self, key: &str) -> Option<&str> {
        self.get(key).map(String::as_str)
    }
    fn env_set(&mut self, key: String, value: String) {
        self.insert(key, value);
    }
}

/// Add GitHub owner routing to `env` when owners are configured. Values the
/// child would inherit from this process (`PATH`, `GIT_CONFIG_COUNT`) are read
/// from the map first and then from this process's environment.
pub fn apply_github_routing<M: EnvMap>(env: &mut M) {
    let Some(owners) = env.env_get(OWNERS_ENV).map(str::to_owned) else {
        return;
    };
    if parse_owners(&owners).is_empty() {
        return;
    }
    let shim_dir = github_auth_bin_dir();
    let shim_dir = match ensure_shim(&shim_dir) {
        Ok(()) => Some(shim_dir),
        Err(error) => {
            tracing::warn!(
                directory = %shim_dir.display(),
                %error,
                "Could not write the GitHub gh routing shim; gh keeps its existing authentication"
            );
            None
        }
    };
    let additions = {
        let lookup = |key: &str| {
            env.env_get(key)
                .map(str::to_owned)
                .or_else(|| std::env::var(key).ok())
        };
        routing_environment(&owners, shim_dir.as_deref(), lookup)
    };
    for (key, value) in additions {
        env.env_set(key, value);
    }
}

/// The variables to add for `owners`. `lookup` returns the value the child
/// would otherwise see for a variable.
pub fn routing_environment(
    owners: &str,
    shim_dir: Option<&Path>,
    lookup: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let owners = parse_owners(owners);
    if owners.is_empty() {
        return Vec::new();
    }
    let mut additions = Vec::new();
    if let Some(shim_dir) = shim_dir {
        let inherited = lookup("PATH").unwrap_or_default();
        let path = merge_paths(shim_dir.as_os_str(), &inherited);
        additions.push(("PATH".to_owned(), path.to_string_lossy().into_owned()));
    }
    let start = match lookup("GIT_CONFIG_COUNT") {
        None => Some(0),
        Some(count) if count.trim().is_empty() => Some(0),
        Some(count) => count.trim().parse::<usize>().ok(),
    };
    let Some(start) = start else {
        tracing::warn!("GIT_CONFIG_COUNT is not a number; skipping GitHub owner routing for Git");
        return additions;
    };
    let mut index = start;
    for owner in owners {
        let helper = credential_helper(&token_env_name(owner));
        let mut spellings = vec![owner.to_owned()];
        let lower = owner.to_ascii_lowercase();
        if lower != owner {
            spellings.push(lower);
        }
        for spelling in spellings {
            let key = format!("credential.https://github.com/{spelling}.helper");
            // An empty value resets helpers inherited from system/global
            // config for this URL prefix only; the next entry adds ours.
            for value in [String::new(), helper.clone()] {
                additions.push((format!("GIT_CONFIG_KEY_{index}"), key.clone()));
                additions.push((format!("GIT_CONFIG_VALUE_{index}"), value));
                index += 1;
            }
        }
    }
    additions.push(("GIT_CONFIG_COUNT".to_owned(), index.to_string()));
    additions
}

/// Inline Git credential helper printing the token held in `variable`. Only
/// the variable name appears in configuration, never the token.
fn credential_helper(variable: &str) -> String {
    format!(
        "!f() {{ test \"$1\" = get || return 0; test -n \"${variable}\" || return 0; \
         printf 'username=x-access-token\\npassword=%s\\n' \"${variable}\"; }}; f"
    )
}

/// The shim script for a given install directory.
pub fn shim_script(shim_dir: &Path) -> String {
    SHIM_TEMPLATE.replace(
        SHIM_DIR_PLACEHOLDER,
        &shell_single_quote(&shim_dir.to_string_lossy()),
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Write `<shim_dir>/gh` if it is missing or out of date. The replacement is
/// atomic, so concurrent spawns never observe a partial script.
#[cfg(unix)]
pub fn ensure_shim(shim_dir: &Path) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::PermissionsExt};

    let script = shim_script(shim_dir);
    let target = shim_dir.join("gh");
    if let Ok(existing) = std::fs::read(&target)
        && existing == script.as_bytes()
        && std::fs::metadata(&target).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    {
        return Ok(());
    }
    std::fs::create_dir_all(shim_dir)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let temporary = shim_dir.join(format!(".gh.{}.{nanos}", std::process::id()));
    let result = (|| {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(script.as_bytes())?;
        file.sync_all()?;
        std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&temporary, &target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

/// The shim is a POSIX shell script; other platforms keep Git routing only.
#[cfg(not(unix))]
pub fn ensure_shim(_shim_dir: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the gh routing shim requires a POSIX shell",
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

    use super::*;

    const TOKEN_A: &str = "synthetic-token-org-a";
    const TOKEN_B: &str = "synthetic-token-org-b";

    struct Fixture {
        _root: tempfile::TempDir,
        shim_dir: PathBuf,
        fake_dir: PathBuf,
        home: PathBuf,
        work: PathBuf,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let shim_dir = root.path().join("shim bin");
        let fake_dir = root.path().join("fake");
        let home = root.path().join("home");
        let work = root.path().join("work");
        for dir in [&fake_dir, &home, &work] {
            std::fs::create_dir_all(dir).unwrap();
        }
        ensure_shim(&shim_dir).unwrap();
        let fake = fake_dir.join("gh");
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf 'token=%s args=%s\\n' \"${GH_TOKEN:-ambient}\" \"$*\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        Fixture {
            _root: root,
            shim_dir,
            fake_dir,
            home,
            work,
        }
    }

    fn system_path() -> String {
        std::env::var("PATH").unwrap_or_default()
    }

    impl Fixture {
        fn command(&self, program: &str, cwd: &Path) -> Command {
            let mut command = Command::new(program);
            command
                .current_dir(cwd)
                .env_clear()
                .env("HOME", &self.home)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env(OWNERS_ENV, "Org-A,org-b")
                .env(token_env_name("Org-A"), TOKEN_A)
                .env(token_env_name("org-b"), TOKEN_B);
            command
        }

        fn gh(&self, cwd: &Path, args: &[&str], extra: &[(&str, &str)]) -> (i32, String, String) {
            let path = format!(
                "{}:{}:{}",
                self.shim_dir.display(),
                self.fake_dir.display(),
                system_path()
            );
            let mut command = self.command(&self.shim_dir.join("gh").to_string_lossy(), cwd);
            command.env("PATH", path).args(args);
            for (key, value) in extra {
                command.env(key, value);
            }
            let output = command.output().unwrap();
            (
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            )
        }

        fn repo(&self, name: &str, remote: &str) -> PathBuf {
            let dir = self.work.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let git = |args: &[&str]| {
                let status = self
                    .command("git", &dir)
                    .env("PATH", system_path())
                    .args(args)
                    .output()
                    .unwrap();
                assert!(status.status.success(), "git {args:?}");
            };
            git(&["init", "-q"]);
            git(&["remote", "add", "origin", remote]);
            dir
        }
    }

    fn token_of(stdout: &str) -> &str {
        stdout
            .strip_prefix("token=")
            .and_then(|rest| rest.split(' ').next())
            .unwrap_or("")
    }

    #[test]
    fn owner_validation_and_env_names() {
        for valid in ["a", "Org-A", "davidvasandani", &"x".repeat(39)] {
            assert!(is_valid_owner(valid), "{valid}");
        }
        for invalid in ["", "-a", "a-", "a_b", "a.b", "a/b", &"x".repeat(40)] {
            assert!(!is_valid_owner(invalid), "{invalid}");
        }
        assert_eq!(token_env_name("Org-A"), "VK_GITHUB_PAT_ORG_A");
        assert_eq!(parse_owners(" a , bad_one ,b"), vec!["a", "b"]);
    }

    #[test]
    fn unconfigured_environment_is_untouched() {
        let mut env = HashMap::from([("PATH".to_owned(), "/bin".to_owned())]);
        let before = env.clone();
        apply_github_routing(&mut env);
        assert_eq!(env, before);
        assert!(routing_environment("", None, |_| None).is_empty());
        assert!(routing_environment("_bad", None, |_| None).is_empty());
    }

    #[test]
    fn routing_environment_prepends_shim_and_continues_git_config() {
        let dir = Path::new("/opt/shim");
        let additions: HashMap<_, _> = routing_environment("Org-A,b", Some(dir), |key| match key {
            "PATH" => Some("/usr/bin:/opt/shim".into()),
            "GIT_CONFIG_COUNT" => Some("2".into()),
            _ => None,
        })
        .into_iter()
        .collect();
        assert_eq!(additions["PATH"], "/opt/shim:/usr/bin");
        // Org-A: two spellings × (reset + helper); b: one spelling.
        assert_eq!(additions["GIT_CONFIG_COUNT"], "8");
        assert_eq!(
            additions["GIT_CONFIG_KEY_2"],
            "credential.https://github.com/Org-A.helper"
        );
        assert_eq!(additions["GIT_CONFIG_VALUE_2"], "");
        assert!(additions["GIT_CONFIG_VALUE_3"].contains("$VK_GITHUB_PAT_ORG_A"));
        assert_eq!(
            additions["GIT_CONFIG_KEY_4"],
            "credential.https://github.com/org-a.helper"
        );
        assert_eq!(
            additions["GIT_CONFIG_KEY_6"],
            "credential.https://github.com/b.helper"
        );
        assert!(!additions.contains_key("GIT_CONFIG_KEY_0"));
        for value in additions.values() {
            assert!(!value.contains("synthetic"));
        }
    }

    #[test]
    fn invalid_existing_count_skips_git_but_keeps_shim() {
        let additions = routing_environment("a", Some(Path::new("/s")), |key| {
            (key == "GIT_CONFIG_COUNT").then(|| "nope".into())
        });
        assert_eq!(additions.len(), 1);
        assert_eq!(additions[0].0, "PATH");
    }

    #[test]
    fn shim_write_is_idempotent_and_quotes_directory() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("it's here");
        ensure_shim(&dir).unwrap();
        let first = std::fs::read_to_string(dir.join("gh")).unwrap();
        assert!(first.contains(r"VK_SHIM_DIR='"));
        assert!(!first.contains(SHIM_DIR_PLACEHOLDER));
        ensure_shim(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("gh")).unwrap(), first);
        let mode = std::fs::metadata(dir.join("gh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn shim_routes_by_cwd_remote_and_explicit_targets() {
        let fx = fixture();
        let repo_a = fx.repo("a", "https://github.com/Org-A/app.git");
        let repo_b = fx.repo("b", "git@github.com:org-b/svc.git");
        let other = fx.repo("c", "https://github.com/someone-else/x");

        assert_eq!(token_of(&fx.gh(&repo_a, &["pr", "list"], &[]).1), TOKEN_A);
        assert_eq!(token_of(&fx.gh(&repo_b, &["pr", "list"], &[]).1), TOKEN_B);
        assert_eq!(
            token_of(
                &fx.gh(&other, &["pr", "list"], &[("GH_TOKEN", "ambient-x")])
                    .1
            ),
            "ambient-x"
        );
        // Explicit targets win over the cwd remote.
        for args in [
            &["pr", "view", "1", "--repo", "org-b/svc"][..],
            &["pr", "view", "1", "-R", "ORG-B/svc"],
            &["pr", "view", "1", "--repo=https://github.com/org-b/svc"],
            &["api", "repos/org-b/svc/pulls"],
            &["api", "-X", "GET", "/repos/org-b/svc"],
            &["pr", "view", "https://github.com/org-b/svc/pull/7"],
            &["repo", "view", "org-b/svc"],
        ] {
            let (code, stdout, _) = fx.gh(&repo_a, args, &[]);
            assert_eq!(code, 0, "{args:?}");
            assert_eq!(token_of(&stdout), TOKEN_B, "{args:?}");
            assert!(
                stdout.contains(&args.join(" ")),
                "args pass through: {stdout}"
            );
        }
        // gh fills `{owner}/{repo}` from the current repository.
        assert_eq!(
            token_of(&fx.gh(&repo_b, &["api", "repos/{owner}/{repo}/pulls"], &[]).1),
            TOKEN_B
        );
        // A configured owner overrides an ambient token.
        assert_eq!(
            token_of(
                &fx.gh(&repo_a, &["pr", "list"], &[("GH_TOKEN", "ambient")])
                    .1
            ),
            TOKEN_A
        );
        // An explicit unconfigured or non-GitHub target keeps ambient auth.
        for args in [
            &["pr", "list", "--repo", "someone-else/x"][..],
            &["pr", "list", "--repo", "ghe.example.com/org-b/svc"],
        ] {
            assert_eq!(
                token_of(&fx.gh(&repo_a, args, &[("GH_TOKEN", "ambient")]).1),
                "ambient",
                "{args:?}"
            );
        }
        // A short OWNER/REPO means another host when GH_HOST says so.
        assert_eq!(
            token_of(
                &fx.gh(
                    &repo_a,
                    &["pr", "list", "--repo", "org-b/svc"],
                    &[("GH_HOST", "ghe.example.com")]
                )
                .1
            ),
            "ambient"
        );
        // Outside any repository nothing is selected.
        assert_eq!(
            token_of(&fx.gh(&fx.work, &["auth", "status"], &[]).1),
            "ambient"
        );
    }

    #[test]
    fn shim_fails_closed_for_configured_owner_without_token() {
        let fx = fixture();
        let repo_a = fx.repo("a", "https://github.com/org-a/app");
        let (code, stdout, stderr) = fx.gh(
            &repo_a,
            &["pr", "list"],
            &[(token_env_name("Org-A").as_str(), "")],
        );
        assert_eq!(code, 78);
        assert!(stdout.is_empty());
        assert!(stderr.contains("org-a"), "{stderr}");
        assert!(!stderr.contains("synthetic"));
    }

    #[test]
    fn shim_reports_missing_real_gh() {
        let fx = fixture();
        let output = fx
            .command(&fx.shim_dir.join("gh").to_string_lossy(), &fx.work)
            // Only the shim itself and system tools (no gh) are on PATH.
            .env(
                "PATH",
                format!("{}:{}", fx.shim_dir.display(), system_path_without_gh()),
            )
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(127));
    }

    fn system_path_without_gh() -> String {
        std::env::split_paths(&system_path())
            .filter(|dir| !dir.join("gh").exists())
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(":")
    }

    #[test]
    fn git_credential_fill_uses_owner_token_and_keeps_other_helpers() {
        let fx = fixture();
        std::fs::write(
            fx.home.join(".gitconfig"),
            "[credential \"https://github.com\"]\n\thelper =\n\thelper = \"!f(){ echo username=u; echo password=existing; }; f\"\n",
        )
        .unwrap();
        let additions = routing_environment("Org-A,org-b", None, |_| None);
        let fill = |url: &str| {
            let mut command = fx.command("git", &fx.work);
            command
                .env("PATH", system_path())
                .env("GIT_TERMINAL_PROMPT", "0")
                .args(["credential", "fill"])
                .envs(additions.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped());
            let mut child = command.spawn().unwrap();
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("url={url}\n\n").as_bytes())
                .unwrap();
            let output = child.wait_with_output().unwrap();
            String::from_utf8_lossy(&output.stdout).into_owned()
        };
        assert!(fill("https://github.com/Org-A/app.git").contains(&format!("password={TOKEN_A}")));
        assert!(fill("https://github.com/org-a/app").contains(&format!("password={TOKEN_A}")));
        assert!(fill("https://github.com/org-b/svc.git").contains(&format!("password={TOKEN_B}")));
        assert!(fill("https://github.com/org-b-other/x.git").contains("password=existing"));
        assert!(fill("https://github.com/someone/x.git").contains("password=existing"));
    }
}
