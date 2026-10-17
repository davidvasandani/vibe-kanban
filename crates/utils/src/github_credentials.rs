//! Per-owner GitHub credentials for the server's own git and `gh` commands.
//!
//! Workspace processes are routed through [`crate::github_auth`]. The server's
//! own operations (push, PR create/status/merge, fetches) instead receive a
//! [`GitHubCredentials`] set resolved per request from Settings → Repositories
//! → GitHub organization tokens, and apply the owner's token to exactly one
//! child process:
//!
//! - git: the owner-scoped helper text from
//!   [`routing_environment`](crate::github_auth::routing_environment) in that
//!   command's `GIT_CONFIG_PARAMETERS`, reading the token from a variable set
//!   on the same command;
//! - gh: `GH_TOKEN`, which gh sends as the `Authorization` header for REST and
//!   GraphQL, with competing token variables removed.
//!
//! An owner with no entry keeps the server's existing credential (fallback).
//! An owner whose configured token could not be read is `Unavailable`, and its
//! commands are refused rather than run with another identity.

use std::{collections::HashMap, fmt, process::Command, sync::Arc};

use crate::github_auth::{OWNERS_ENV, is_valid_owner, routing_environment, token_env_name};

const SETTINGS_PATH: &str = "Settings → Repositories → GitHub organization tokens";
/// Token variables gh reads ahead of (or instead of) `GH_TOKEN`.
const COMPETING_GH_TOKENS: [&str; 3] = [
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

/// `owner/repo` of a github.com repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepoRef {
    pub owner: String,
    pub repo: String,
}

/// Owner and repository of a github.com remote or PR URL: HTTPS (with
/// userinfo, `.git`, extra path such as `/pull/12`), `ssh://`, or scp-like
/// `git@github.com:owner/repo`. Other hosts return `None`.
pub fn parse_github_repo(url: &str) -> Option<GitHubRepoRef> {
    let url = url.trim();
    let (authority, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let scheme = scheme.to_ascii_lowercase();
            if !matches!(
                scheme.as_str(),
                "https" | "http" | "ssh" | "git" | "git+ssh"
            ) {
                return None;
            }
            let (authority, path) = rest.split_once('/')?;
            let host_port = authority.rsplit('@').next()?;
            (host_port.split(':').next()?, path)
        }
        None => {
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') {
                return None;
            }
            (authority.rsplit('@').next()?, path)
        }
    };
    if !authority.eq_ignore_ascii_case("github.com")
        && !authority.eq_ignore_ascii_case("www.github.com")
    {
        return None;
    }
    let mut segments = path.trim_start_matches('/').split('/');
    let owner = segments.next()?;
    let repo = segments.next()?.split(['?', '#']).next()?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    let repo_valid = !repo.is_empty()
        && repo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    (is_valid_owner(owner) && repo_valid).then(|| GitHubRepoRef {
        owner: owner.to_owned(),
        repo: repo.to_owned(),
    })
}

/// A resolved token. `Debug` never prints the value.
#[derive(Clone)]
pub struct GitHubToken(Arc<str>);

impl GitHubToken {
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GitHubToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GitHubToken(<redacted>)")
    }
}

/// What the org-token table yielded for one configured owner.
#[derive(Debug, Clone)]
pub enum OwnerCredential {
    OrgToken(GitHubToken),
    /// Configured but unreadable; the reason never contains the value.
    Unavailable(String),
}

/// Credentials for the owners one request may contact, keyed by lower-cased
/// owner. Empty (the default) means every owner uses the fallback.
#[derive(Debug, Clone, Default)]
pub struct GitHubCredentials {
    owners: HashMap<String, (String, OwnerCredential)>,
}

impl GitHubCredentials {
    pub fn insert(&mut self, owner: &str, credential: OwnerCredential) {
        self.owners
            .insert(owner.to_ascii_lowercase(), (owner.to_owned(), credential));
    }

    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }

    /// The credential for `owner` (case-insensitive); `None` is a target whose
    /// owner is unknown, which always uses the fallback.
    pub fn select_owner(&self, owner: Option<&str>) -> CredentialSelection {
        let Some(owner) = owner.filter(|owner| is_valid_owner(owner)) else {
            return CredentialSelection::fallback(None, None);
        };
        let kind = match self.owners.get(&owner.to_ascii_lowercase()) {
            Some((_, OwnerCredential::OrgToken(token))) => SelectionKind::OrgToken(token.clone()),
            Some((_, OwnerCredential::Unavailable(reason))) => {
                SelectionKind::Unavailable(reason.clone())
            }
            None => SelectionKind::Fallback,
        };
        CredentialSelection {
            owner: Some(owner.to_owned()),
            configured_owner: self
                .owners
                .get(&owner.to_ascii_lowercase())
                .map(|(configured, _)| configured.clone()),
            repo: None,
            other_spellings: Vec::new(),
            kind,
        }
    }

    /// The credential for the owner of a GitHub URL; other hosts fall back.
    pub fn select_url(&self, url: &str) -> CredentialSelection {
        match parse_github_repo(url) {
            Some(target) => {
                let mut selection = self.select_owner(Some(&target.owner));
                selection.repo = Some(target.repo);
                selection
            }
            None => CredentialSelection::fallback(None, None),
        }
    }
}

#[derive(Debug, Clone)]
enum SelectionKind {
    OrgToken(GitHubToken),
    Unavailable(String),
    Fallback,
}

/// The credential chosen for one command.
#[derive(Debug, Clone)]
pub struct CredentialSelection {
    /// Owner as named by the target (URL or repository info).
    owner: Option<String>,
    /// Owner as entered in Settings, when configured.
    configured_owner: Option<String>,
    repo: Option<String>,
    /// Further spellings of the same owner that git may see in URLs.
    other_spellings: Vec<String>,
    kind: SelectionKind,
}

impl CredentialSelection {
    fn fallback(owner: Option<String>, repo: Option<String>) -> Self {
        Self {
            owner,
            configured_owner: None,
            repo,
            other_spellings: Vec::new(),
            kind: SelectionKind::Fallback,
        }
    }

    /// Also cover these spellings of the owner (case variants used by the
    /// checkout's remotes); git matches URL prefixes case-sensitively.
    pub fn with_owner_spellings(mut self, spellings: impl IntoIterator<Item = String>) -> Self {
        if let Some(owner) = self.owner.clone() {
            self.other_spellings.extend(
                spellings
                    .into_iter()
                    .filter(|spelling| spelling.eq_ignore_ascii_case(&owner)),
            );
        }
        self
    }

    /// Name the repository in attributed errors.
    pub fn with_repo(mut self, repo: Option<&str>) -> Self {
        if let Some(repo) = repo.filter(|repo| !repo.is_empty()) {
            self.repo = Some(repo.to_owned());
        }
        self
    }

    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }

    pub fn is_org_token(&self) -> bool {
        matches!(self.kind, SelectionKind::OrgToken(_))
    }

    /// `org-token`, `unavailable` or `fallback`, for logs.
    pub fn source_label(&self) -> &'static str {
        match self.kind {
            SelectionKind::OrgToken(_) => "org-token",
            SelectionKind::Unavailable(_) => "unavailable",
            SelectionKind::Fallback => "fallback",
        }
    }

    /// Prepare `command` (a git invocation) to contact `url`, returning the URL
    /// to pass to git. With an org token the URL becomes a credential-free
    /// `https://github.com/<owner>/<repo>.git`: SSH keys, and userinfo embedded
    /// in an HTTPS remote, would otherwise bypass the credential helper.
    /// `Err` (unavailable) means the command must not run.
    pub fn apply_git(&self, command: &mut Command, url: &str) -> Result<String, String> {
        let token = match &self.kind {
            SelectionKind::Fallback => return Ok(url.to_owned()),
            SelectionKind::Unavailable(_) => return Err(self.unavailable_message()),
            SelectionKind::OrgToken(token) => token,
        };
        for (key, value) in self.git_environment(token, false) {
            command.env(key, value);
        }
        command.env_remove(OWNERS_ENV);
        Ok(match parse_github_repo(url) {
            Some(target) if !is_plain_https(url) => {
                format!("https://github.com/{}/{}.git", target.owner, target.repo)
            }
            _ => url.to_owned(),
        })
    }

    /// Prepare `command` (a gh invocation). The org token becomes `GH_TOKEN`
    /// and wins over any ambient token or `gh auth` login. The git variables
    /// are added too, with SSH URLs for the owner rewritten to HTTPS, so gh's
    /// own git calls (`pr checkout` fetching through an SSH remote) use the
    /// same token.
    pub fn apply_gh(&self, command: &mut Command) -> Result<(), String> {
        let token = match &self.kind {
            SelectionKind::Fallback => return Ok(()),
            SelectionKind::Unavailable(_) => return Err(self.unavailable_message()),
            SelectionKind::OrgToken(token) => token,
        };
        for (key, value) in self.git_environment(token, true) {
            command.env(key, value);
        }
        for key in COMPETING_GH_TOKENS {
            command.env_remove(key);
        }
        command.env_remove(OWNERS_ENV);
        command.env("GH_TOKEN", token.expose());
        Ok(())
    }

    fn git_environment(&self, token: &GitHubToken, rewrite_ssh: bool) -> Vec<(String, String)> {
        // Git matches credential URL contexts case-sensitively, so cover the
        // spelling the target used and the one entered in Settings
        // (`routing_environment` adds the lower-cased form of each).
        let mut spellings: Vec<String> = self.owner.iter().cloned().collect();
        if let Some(configured) = &self.configured_owner
            && !spellings.contains(configured)
        {
            spellings.push(configured.clone());
        }
        for spelling in &self.other_spellings {
            if !spellings.contains(spelling) {
                spellings.push(spelling.clone());
            }
        }
        let Some(first) = spellings.first().cloned() else {
            return Vec::new();
        };
        let mut environment =
            routing_environment(&spellings.join(","), None, |key| std::env::var(key).ok());
        for spelling in spellings.clone() {
            let lower = spelling.to_ascii_lowercase();
            if !spellings.contains(&lower) {
                spellings.push(lower);
            }
        }
        // Keep the owner's HTTPS URLs on HTTPS. Git applies the *longest*
        // matching `insteadOf`/`pushInsteadOf`, so these identity rewrites beat
        // an inherited `url."git@github.com:".insteadOf=https://github.com/`
        // that would send the command to SSH, past the credential helper.
        let mut rewrites = Vec::new();
        for owner in &spellings {
            let base = format!("https://github.com/{owner}/");
            for variable in ["insteadOf", "pushInsteadOf"] {
                rewrites.push(format!("url.{base}.{variable}={base}"));
            }
            // An inherited Authorization header (e.g. persisted by a checkout
            // action) would be sent instead of the helper's credential. An
            // empty value resets the list; this owner-scoped URL outranks a
            // host-wide `http.https://github.com/.extraHeader`.
            rewrites.push(format!("http.{base}.extraHeader="));
            // Repository-scoped rewrites and headers are more specific still.
            // (Git keeps the first of equally long rewrites, so an inherited
            // rule naming exactly the full repository URL is not overridden.)
            if let Some(repo) = &self.repo {
                for url in [format!("{base}{repo}"), format!("{base}{repo}.git")] {
                    for variable in ["insteadOf", "pushInsteadOf"] {
                        rewrites.push(format!("url.{url}.{variable}={url}"));
                    }
                    rewrites.push(format!("http.{url}.extraHeader="));
                }
            }
            // gh's own fetches go through the checkout's remotes, which may be
            // SSH; route the owner's SSH URLs to HTTPS too.
            if rewrite_ssh {
                // Every SSH form `parse_github_repo` accepts.
                for host in ["github.com", "www.github.com"] {
                    for ssh in [
                        format!("git@{host}:{owner}/"),
                        format!("{host}:{owner}/"),
                        format!("ssh://git@{host}/{owner}/"),
                        format!("ssh://git@{host}:22/{owner}/"),
                        format!("ssh://{host}/{owner}/"),
                        format!("ssh://{host}:22/{owner}/"),
                        format!("git+ssh://git@{host}/{owner}/"),
                    ] {
                        rewrites.push(format!("url.{base}.insteadOf={ssh}"));
                    }
                }
            }
        }
        if let Some((_, parameters)) = environment
            .iter_mut()
            .find(|(key, _)| key == GIT_CONFIG_PARAMETERS)
        {
            for entry in rewrites {
                parameters.push(' ');
                parameters.push_str(&sq_quote(&entry));
            }
        }
        environment.push((token_env_name(&first), token.expose().to_owned()));
        environment
    }

    fn target(&self) -> String {
        match (&self.owner, &self.repo) {
            (Some(owner), Some(repo)) => format!("{owner}/{repo}"),
            (Some(owner), None) => format!("{owner}'s repositories"),
            _ => "the repository".to_owned(),
        }
    }

    fn unavailable_message(&self) -> String {
        let reason = match &self.kind {
            SelectionKind::Unavailable(reason) => reason.as_str(),
            _ => "",
        };
        format!(
            "the {} org token is configured but unavailable ({SETTINGS_PATH}): {reason}",
            self.owner.as_deref().unwrap_or("GitHub")
        )
    }

    /// Prefix an authentication or permission failure with the owner and the
    /// credential source, so the message names the credential to fix.
    pub fn attribute(&self, detail: &str) -> String {
        match (&self.kind, &self.owner) {
            (SelectionKind::OrgToken(_), Some(owner)) => format!(
                "the {owner} org token ({SETTINGS_PATH}) was rejected or lacks access to {}: {detail}",
                self.target()
            ),
            (SelectionKind::Unavailable(_), _) => {
                format!("{}: {detail}", self.unavailable_message())
            }
            (_, Some(owner)) => format!(
                "no org token for owner {owner}; used the server's fallback credential, which \
                 was rejected or lacks access to {}. Add a {owner} token in {SETTINGS_PATH}: \
                 {detail}",
                self.target()
            ),
            _ => detail.to_owned(),
        }
    }
}

const GIT_CONFIG_PARAMETERS: &str = "GIT_CONFIG_PARAMETERS";

/// Already in the exact form the owner-scoped helper context matches:
/// `https://github.com/…` with no userinfo, port or `www.` host.
fn is_plain_https(url: &str) -> bool {
    url.trim().starts_with("https://github.com/")
}

/// Quote a `GIT_CONFIG_PARAMETERS` entry the way git's `sq_quote` does.
fn sq_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Whether git stderr reports an authentication or permission failure.
pub fn looks_like_git_auth_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "authentication failed",
        "could not read username",
        "could not read password",
        "invalid username or password",
        "returned error: 401",
        "returned error: 403",
        "write access to repository not granted",
        "repository not found",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || (lower.contains("permission to") && lower.contains("denied"))
}

/// Whether gh stderr reports an authentication or permission failure.
pub fn looks_like_gh_auth_failure(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    [
        "http 401",
        "http 403",
        "bad credentials",
        "resource not accessible",
        "must authenticate",
        "requires authentication",
        "authentication failed",
        "gh auth login",
        "could not resolve to a repository",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
        || looks_like_git_auth_failure(message)
}

#[cfg(all(test, unix))]
mod tests {
    use std::{io::Write, process::Stdio};

    use super::*;

    const TOKEN: &str = "synthetic-org-token";

    fn repo(owner: &str, repo: &str) -> Option<GitHubRepoRef> {
        Some(GitHubRepoRef {
            owner: owner.into(),
            repo: repo.into(),
        })
    }

    fn credentials() -> GitHubCredentials {
        let mut credentials = GitHubCredentials::default();
        credentials.insert(
            "SweetGreen",
            OwnerCredential::OrgToken(GitHubToken::new(TOKEN)),
        );
        credentials.insert(
            "broken",
            OwnerCredential::Unavailable("1Password lookup failed".into()),
        );
        credentials
    }

    #[test]
    fn parses_https_ssh_scp_and_pr_urls() {
        let cases = [
            (
                "https://github.com/sweetgreen/platform-ops",
                repo("sweetgreen", "platform-ops"),
            ),
            (
                "https://github.com/sweetgreen/platform-ops.git",
                repo("sweetgreen", "platform-ops"),
            ),
            (
                "https://x-access-token:abc@github.com/o/r.git/",
                repo("o", "r"),
            ),
            ("http://GitHub.com/o/r", repo("o", "r")),
            ("https://www.github.com/o/r", repo("o", "r")),
            ("https://github.com/o/r/pull/12", repo("o", "r")),
            ("https://github.com/o/r.js?tab=x", repo("o", "r.js")),
            ("ssh://git@github.com/o/r.git", repo("o", "r")),
            ("ssh://git@github.com:22/o/r", repo("o", "r")),
            ("git@github.com:Org-A/app.git", repo("Org-A", "app")),
            ("github.com:o/r", repo("o", "r")),
            ("  git@GITHUB.COM:o/r.git\n", repo("o", "r")),
            ("https://gitlab.com/o/r", None),
            ("https://ghe.example.com/o/r", None),
            ("git@ghe.example.com:o/r.git", None),
            ("https://github.com/o", None),
            ("https://github.com/bad_owner/r", None),
            ("/srv/repos/local", None),
            ("file:///srv/repos/github.com/o/r", None),
            ("C:\\repos\\x", None),
        ];
        for (url, expected) in cases {
            assert_eq!(parse_github_repo(url), expected, "{url}");
        }
    }

    #[test]
    fn selection_is_case_insensitive_and_falls_back_for_unknown_owners() {
        let credentials = credentials();
        let selected = credentials.select_url("git@github.com:sweetgreen/platform-ops.git");
        assert_eq!(selected.source_label(), "org-token");
        assert_eq!(selected.owner(), Some("sweetgreen"));
        assert_eq!(
            credentials.select_owner(Some("SWEETGREEN")).source_label(),
            "org-token"
        );
        assert_eq!(
            credentials
                .select_url("https://github.com/someone/x")
                .source_label(),
            "fallback"
        );
        assert_eq!(
            credentials
                .select_url("https://ghe.example.com/sweetgreen/x")
                .source_label(),
            "fallback"
        );
        assert_eq!(credentials.select_owner(None).source_label(), "fallback");
        assert_eq!(
            credentials
                .select_url("https://github.com/broken/x")
                .source_label(),
            "unavailable"
        );
    }

    #[test]
    fn debug_output_never_contains_the_token() {
        let credentials = credentials();
        let rendered = format!(
            "{credentials:?} {:?}",
            credentials.select_owner(Some("sweetgreen"))
        );
        assert!(!rendered.contains(TOKEN), "{rendered}");
    }

    #[test]
    fn unavailable_refuses_to_prepare_commands() {
        let selection = credentials().select_url("https://github.com/broken/x");
        let mut git = Command::new("git");
        let error = selection
            .apply_git(&mut git, "https://github.com/broken/x")
            .unwrap_err();
        assert!(error.contains("broken org token is configured but unavailable"));
        assert!(error.contains("1Password lookup failed"));
        assert!(selection.apply_gh(&mut Command::new("gh")).is_err());
    }

    #[test]
    fn org_tokens_rewrite_ssh_to_https_and_fallback_keeps_the_url() {
        let credentials = credentials();
        let ssh = "git@github.com:sweetgreen/platform-ops.git";
        let mut command = Command::new("git");
        assert_eq!(
            credentials
                .select_url(ssh)
                .apply_git(&mut command, ssh)
                .unwrap(),
            "https://github.com/sweetgreen/platform-ops.git"
        );
        // Embedded userinfo would bypass the helper; plain HTTPS is kept.
        for url in [
            "https://user:old-token@github.com/sweetgreen/platform-ops.git",
            "http://github.com/sweetgreen/platform-ops",
            "https://www.github.com/sweetgreen/platform-ops",
            "https://github.com:443/sweetgreen/platform-ops.git",
        ] {
            let mut command = Command::new("git");
            assert_eq!(
                credentials
                    .select_url(url)
                    .apply_git(&mut command, url)
                    .unwrap(),
                "https://github.com/sweetgreen/platform-ops.git",
                "{url}"
            );
        }
        let plain = "https://github.com/sweetgreen/platform-ops";
        let mut command = Command::new("git");
        assert_eq!(
            credentials
                .select_url(plain)
                .apply_git(&mut command, plain)
                .unwrap(),
            plain
        );
        let other = "git@github.com:someone/x.git";
        let mut command = Command::new("git");
        assert_eq!(
            credentials
                .select_url(other)
                .apply_git(&mut command, other)
                .unwrap(),
            other
        );
        assert_eq!(command.get_envs().count(), 0, "fallback adds nothing");
    }

    fn git_credential_fill(
        home: &std::path::Path,
        selection: &CredentialSelection,
        url: &str,
    ) -> String {
        let mut command = Command::new("git");
        command
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(home)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env(OWNERS_ENV, "sweetgreen")
            .args(["credential", "fill"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let url = selection.apply_git(&mut command, url).unwrap();
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("url={url}\n\n").as_bytes())
            .unwrap();
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).into_owned()
    }

    #[test]
    fn real_git_uses_the_org_token_over_an_ambient_helper() {
        let home = tempfile::tempdir().unwrap();
        // The production shape: a host-wide helper for all of github.com.
        std::fs::write(
            home.path().join(".gitconfig"),
            "[credential \"https://github.com\"]\n\thelper =\n\thelper = \"!f(){ echo username=u; echo password=ambient; }; f\"\n",
        )
        .unwrap();
        let credentials = credentials();
        for url in [
            "https://github.com/sweetgreen/platform-ops.git",
            "https://github.com/SweetGreen/platform-ops",
            "git@github.com:sweetgreen/platform-ops.git",
        ] {
            let output = git_credential_fill(home.path(), &credentials.select_url(url), url);
            assert!(
                output.contains(&format!("password={TOKEN}")),
                "{url}: {output}"
            );
        }
        // A remote spelling outside the canonical/configured/lower-case set is
        // covered once named (as `gh pr checkout` does for its remotes).
        let shouting = "https://github.com/SWEETGREEN/platform-ops.git";
        let selection = credentials
            .select_owner(Some("sweetgreen"))
            .with_owner_spellings(["SWEETGREEN".to_string(), "someone".to_string()]);
        let output = git_credential_fill(home.path(), &selection, shouting);
        assert!(output.contains(&format!("password={TOKEN}")), "{output}");
        let other = "https://github.com/someone/x.git";
        let output = git_credential_fill(home.path(), &credentials.select_url(other), other);
        assert!(output.contains("password=ambient"), "{output}");
    }

    #[test]
    fn gh_receives_the_org_token_instead_of_ambient_tokens() {
        let credentials = credentials();
        let run = |selection: &CredentialSelection| {
            let mut command = Command::new("sh");
            command
                .args([
                    "-c",
                    "printf '%s|%s|%s' \"${GH_TOKEN:-none}\" \"${GITHUB_TOKEN:-none}\" \"${VK_GITHUB_ROUTED_OWNERS:-none}\"",
                ])
                .env("GH_TOKEN", "ambient-gh")
                .env("GITHUB_TOKEN", "ambient-github")
                .env(OWNERS_ENV, "sweetgreen");
            selection.apply_gh(&mut command).unwrap();
            String::from_utf8(command.output().unwrap().stdout).unwrap()
        };
        assert_eq!(
            run(&credentials.select_owner(Some("sweetgreen"))),
            format!("{TOKEN}|none|none")
        );
        assert_eq!(
            run(&credentials.select_owner(Some("someone"))),
            "ambient-gh|ambient-github|sweetgreen"
        );
    }

    #[test]
    fn gh_git_calls_rewrite_the_owners_ssh_urls_to_https() {
        let home = tempfile::tempdir().unwrap();
        let credentials = credentials();
        let get_url = |selection: &CredentialSelection, url: &str| {
            let mut command = Command::new("git");
            command
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", home.path())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .current_dir(home.path())
                .args(["ls-remote", "--get-url", url]);
            selection.apply_gh(&mut command).unwrap();
            String::from_utf8(command.output().unwrap().stdout)
                .unwrap()
                .trim()
                .to_owned()
        };
        let selection = credentials.select_owner(Some("sweetgreen"));
        assert_eq!(
            get_url(&selection, "git@github.com:sweetgreen/platform-ops.git"),
            "https://github.com/sweetgreen/platform-ops.git"
        );
        assert_eq!(
            get_url(&selection, "ssh://git@github.com/SweetGreen/platform-ops"),
            "https://github.com/SweetGreen/platform-ops"
        );
        for ssh in [
            "ssh://git@github.com:22/sweetgreen/platform-ops.git",
            "git+ssh://git@github.com/sweetgreen/platform-ops.git",
            "github.com:sweetgreen/platform-ops.git",
        ] {
            assert_eq!(
                get_url(&selection, ssh),
                "https://github.com/sweetgreen/platform-ops.git",
                "{ssh}"
            );
        }
        assert_eq!(
            get_url(&selection, "git@github.com:someone/x.git"),
            "git@github.com:someone/x.git"
        );
    }

    #[test]
    fn inherited_https_to_ssh_rewrites_cannot_bypass_the_org_token() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join(".gitconfig"),
            "[url \"git@github.com:\"]\n\tinsteadOf = https://github.com/\n\tpushInsteadOf = https://github.com/\n",
        )
        .unwrap();
        let repo = home.path().join("repo");
        let git = |selection: Option<&CredentialSelection>, args: &[&str]| {
            let mut command = Command::new("git");
            command
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", home.path())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .args(args);
            command.current_dir(if repo.exists() {
                repo.as_path()
            } else {
                home.path()
            });
            if let Some(selection) = selection {
                selection
                    .apply_git(&mut command, "https://github.com/sweetgreen/x")
                    .unwrap();
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        };
        let credentials = credentials();
        let org = credentials.select_owner(Some("sweetgreen"));
        let url = "https://github.com/sweetgreen/x";
        assert_eq!(git(Some(&org), &["ls-remote", "--get-url", url]), url);
        // Without an org token the inherited rewrite still applies.
        assert_eq!(
            git(None, &["ls-remote", "--get-url", url]),
            "git@github.com:sweetgreen/x"
        );
        let other = credentials.select_owner(Some("someone"));
        assert_eq!(
            git(
                Some(&other),
                &["ls-remote", "--get-url", "https://github.com/someone/y"]
            ),
            "git@github.com:someone/y"
        );
        git(None, &["init", "-q", repo.to_str().unwrap()]);
        git(None, &["remote", "add", "origin", url]);
        assert_eq!(
            git(Some(&org), &["remote", "get-url", "--push", "origin"]),
            url
        );
        assert_eq!(
            git(None, &["remote", "get-url", "--push", "origin"]),
            "git@github.com:sweetgreen/x"
        );
    }

    #[test]
    fn inherited_authorization_headers_are_reset_for_org_token_owners() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(
            home.path().join(".gitconfig"),
            "[http \"https://github.com/\"]\n\textraHeader = AUTHORIZATION: basic AMBIENT\n[http \"https://github.com/sweetgreen/x.git\"]\n\textraHeader = AUTHORIZATION: basic REPO\n[http \"https://github.com/sweetgreen/x\"]\n\textraHeader = AUTHORIZATION: basic REPO\n",
        )
        .unwrap();
        let header = |selection: &CredentialSelection, url: &str| {
            let mut command = Command::new("git");
            command
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", home.path())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                // Not the process cwd: a CI checkout persists its own extraHeader.
                .current_dir(home.path())
                .args(["config", "--get-urlmatch", "http.extraheader", url]);
            selection.apply_git(&mut command, url).unwrap();
            String::from_utf8(command.output().unwrap().stdout)
                .unwrap()
                .trim()
                .to_owned()
        };
        let credentials = credentials();
        for url in [
            "https://github.com/sweetgreen/x",
            "https://github.com/sweetgreen/x.git",
        ] {
            assert_eq!(header(&credentials.select_url(url), url), "", "{url}");
        }
        assert_eq!(
            header(
                &credentials.select_owner(Some("someone")),
                "https://github.com/someone/x"
            ),
            "AUTHORIZATION: basic AMBIENT"
        );
    }

    #[test]
    fn attribution_names_owner_and_source() {
        let credentials = credentials();
        let detail = "remote: Write access to repository not granted. 403";
        let org = credentials
            .select_url("https://github.com/sweetgreen/platform-ops")
            .attribute(detail);
        assert!(
            org.starts_with(
                "the sweetgreen org token (Settings → Repositories → GitHub organization tokens) \
                 was rejected or lacks access to sweetgreen/platform-ops"
            ),
            "{org}"
        );
        assert!(org.ends_with(detail));
        let fallback = credentials
            .select_url("https://github.com/someone/x")
            .attribute(detail);
        assert!(
            fallback.starts_with(
                "no org token for owner someone; used the server's fallback credential"
            ),
            "{fallback}"
        );
        assert_eq!(credentials.select_owner(None).attribute(detail), detail);
        let unavailable = credentials.select_owner(Some("broken")).attribute(detail);
        assert!(unavailable.contains("configured but unavailable"));
    }

    #[test]
    fn auth_failure_detection() {
        for message in [
            "remote: Write access to repository not granted.\nfatal: unable to access 'https://github.com/o/r/': The requested URL returned error: 403",
            "fatal: Authentication failed for 'https://github.com/o/r/'",
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled",
            "remote: Permission to o/r.git denied to someone.",
            "remote: Repository not found.",
        ] {
            assert!(looks_like_git_auth_failure(message), "{message}");
        }
        assert!(!looks_like_git_auth_failure(
            "! [rejected] main -> main (non-fast-forward)"
        ));
        for message in [
            "HTTP 401: Bad credentials (https://api.github.com/graphql)",
            "gh: Resource not accessible by personal access token (HTTP 403)",
            "GraphQL: Could not resolve to a Repository with the name 'o/r'.",
        ] {
            assert!(looks_like_gh_auth_failure(message), "{message}");
        }
        assert!(!looks_like_gh_auth_failure("HTTP 422: Validation Failed"));
    }
}
