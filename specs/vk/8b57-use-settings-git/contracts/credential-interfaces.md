# Contracts (Rust interfaces)

## utils::github_credentials (new)
```rust
pub struct GitHubRepoRef { pub owner: String, pub repo: String }
pub fn parse_github_repo(url: &str) -> Option<GitHubRepoRef>;

#[derive(Clone)] pub struct GitHubToken(/* redacted Debug */);
pub enum OwnerCredential { OrgToken(GitHubToken), Unavailable(String) }

#[derive(Clone, Default)] pub struct GitHubCredentials { /* … */ }
impl GitHubCredentials {
    pub fn insert(&mut self, owner: &str, credential: OwnerCredential);
    pub fn select_owner(&self, owner: Option<&str>) -> CredentialSelection;
    pub fn select_url(&self, url: &str) -> CredentialSelection;
}

pub enum CredentialSelection {
    OrgToken { owner: String, token: GitHubToken },
    Unavailable { owner: String, reason: String },
    Fallback { owner: Option<String> },
}
impl CredentialSelection {
    pub fn source_label(&self) -> &'static str;           // org-token | unavailable | fallback
    pub fn apply_git(&self, cmd: &mut Command, url: &str) -> Result<OsString, String>; // URL to contact
    pub fn apply_gh(&self, cmd: &mut Command) -> Result<(), String>;
    pub fn attribute(&self, repo: Option<&str>, detail: &str) -> String;
}
pub fn looks_like_git_auth_failure(message: &str) -> bool;
pub fn looks_like_gh_auth_failure(message: &str) -> bool;
```

## git
```rust
GitCliError::CredentialUnavailable(String)          // new variant
GitCli::push_with(&self, repo, url, branch, force, &GitHubCredentials)
GitCli::fetch_with_refspec_with(&self, repo, url, refspec, &GitHubCredentials)
GitCli::check_remote_branch_exists_with(&self, repo, url, branch, &GitHubCredentials)
GitCli::remote_branch_oid(&self, repo, url, branch, &GitHubCredentials) -> Result<Option<String>, _>
GitService::push_to_remote(.., force, creds: &GitHubCredentials)          // changed
GitService::push_to_remote_if_needed(worktree, branch, creds) -> Result<PushOutcome, _> // new
GitService::check_remote_branch_exists(.., creds)                         // changed
GitService::get_remote_branch_status(.., creds)                           // changed
GitService::rebase_branch(.., creds)                                      // changed
pub enum PushOutcome { Pushed, AlreadyUpToDate }
```

## git-host
```rust
GhCli::with_credentials(GitHubCredentials) -> GhCli
GitHubProvider::with_credentials(GitHubCredentials) -> Self
GitHostService::from_url_with_credentials(url, GitHubCredentials) -> Result<Self, GitHostError>
```

## services::github_credentials (new)
```rust
pub async fn resolve_for_urls<'a>(pool: &SqlitePool, urls: impl IntoIterator<Item = &'a str>) -> GitHubCredentials;
pub async fn resolve_for_repo(pool: &SqlitePool, git: &GitService, repo_path: &Path) -> GitHubCredentials;
pub fn invalidate_cache();
```

## HTTP / MCP
No route or JSON shape changes. Messages change: auth failures carry the
owner and source attribution. `PrToolError::CliNotLoggedIn` (not exported to
TS) gains `detail`.
