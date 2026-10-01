# Implementation plan: server-side GitHub credentials from the org-token table

Task `vk/8b57-use-settings-git`. See `SPEC.md` for requirements (FR-1…FR-9).

## Step 1: `utils::github_credentials` (new module, pure, no I/O)

File: `crates/utils/src/github_credentials.rs`, exported from `lib.rs`.

- `GitHubRepoRef { owner, repo }` and `parse_github_repo(url) ->
  Option<GitHubRepoRef>`. Accepts https/http (with userinfo, `.git`, trailing
  `/`, extra path such as `/pull/12`), `ssh://[user@]github.com[:port]/o/r`,
  scp-like `[user@]github.com:o/r`. The host matches `github.com` or
  `www.github.com` case-insensitively. The owner must pass `is_valid_owner`,
  and the repo must be non-empty.
- `GitHubToken(Arc<str>)` with a redacting `Debug`.
- `OwnerCredential { OrgToken(GitHubToken), Unavailable(String) }`.
- `GitHubCredentials`: a map from lowercase owner to (owner as entered,
  `OwnerCredential`). Derives `Default` and `Clone`, with a redacting `Debug`.
  Has `insert`, `select_owner(owner)`, `select_url(url)`.
- `CredentialSelection` (owned): `OrgToken { owner, token }`, `Unavailable {
  owner, reason }`, `Fallback { owner: Option<String> }`. Methods:
  - `source_label()` returns `org-token` / `unavailable` / `fallback`.
  - `git_auth(url) -> GitCommandAuth { url, set, remove }`. `OrgToken`
    yields the PAT variable plus the `GIT_CONFIG_PARAMETERS` produced by
    `github_auth::routing_environment(owner, None, process lookup)`, removes
    `OWNERS_ENV`, and rewrites SSH and scp URLs to HTTPS. `Fallback` yields
    no changes.
  - `apply_git(&mut Command, url) -> Result<OsString url, String>`.
    `Unavailable` returns an error.
  - `gh_env()` / `apply_gh(&mut Command)`: `GH_TOKEN` plus the git auth;
    removes `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN`
    and `OWNERS_ENV`.
  - `attribute(repo: Option<&str>, detail) -> String`: the FR-6 wording.
- `looks_like_git_auth_failure(msg)` and `looks_like_gh_auth_failure(msg,
  exit_code)`.
- Unit tests: parsing table, case-insensitive selection, fallback, the env
  produced, a real `git credential fill` (temp HOME, an ambient global helper
  that prints `ambient`) returning the org token for the owner and `ambient`
  for others, SSH rewrite, `gh` env override against an ambient
  `GITHUB_TOKEN`/`GH_TOKEN` via `sh -c`, attribution wording, and redacting
  `Debug`.

## Step 2: `git` crate: credentialed network commands

- `GitCliError::CredentialUnavailable(String)`.
- `classify_cli_error`: also treat `403`, `write access to repository not
  granted`, `permission to … denied`, `the requested url returned error: 401`
  as `AuthFailed`.
- `GitCli::push_with`, `fetch_with_refspec_with`, `check_remote_branch_exists_with`
  and a new `remote_branch_oid_with` take `&GitHubCredentials`. They select by
  URL, apply the auth, run, and on `AuthFailed` re-wrap the message with the
  attribution. The existing names delegate with an empty set (local-path
  callers in workspace-manager and the tests).
- `GitService`: `push_to_remote`, `check_remote_branch_exists`,
  `get_remote_branch_status`, `rebase_branch` gain `creds:
  &GitHubCredentials`. `fetch_*` is threaded through. Add
  `push_to_remote_if_needed(worktree, branch, creds) -> PushOutcome
  { Pushed, AlreadyUpToDate }` (FR-7). It compares `ls-remote` against the
  local branch tip, and when they match it records the tracking ref and
  upstream as a push would. The tracking-ref update is factored into a helper.
- Tests (`crates/git/tests` or unit): skip versus push against a local bare
  remote, and that an `Unavailable` selection refuses to run.

## Step 3: `git-host`: credentialed `gh`

- `GhCli { credentials: GitHubCredentials }`, `GhCli::with_credentials`.
  `run(owner: Option<&str>, args, dir)` applies `select_owner`. `Unavailable`
  becomes `GhCliError::AuthFailed(attributed)` without spawning. On failure, an
  auth-looking stderr is prefixed with the attribution and still classified as
  before (so a 403 stays `InsufficientPermissions` → check coverage
  `Forbidden`).
- Each method passes its owner: `get_repo_info` (parse URL), `view_pr`
  (parse PR URL), and the rest from `GitHubRepoInfo` / `owner` args.
- `GitHubProvider::with_credentials`. `GitHostService::from_url_with_credentials`.
  `from_url` delegates with an empty set.

## Step 4: `services::github_credentials` (resolver)

File: `crates/services/src/services/github_credentials.rs`.

- `resolve_for_urls(pool, urls) -> GitHubCredentials`, and `resolve_for_repo(pool,
  git, repo_path)`, which takes all remotes from `git.list_remotes`.
- Parse owners. Return early with an empty set when there are none. List rows
  (DB error → every requested owner `Unavailable`). Match case-insensitively.
  Load the store only if a row matched. Decrypt (`Undecryptable` →
  `Unavailable`). Literal → `OrgToken`. Reference → cached op resolution
  (60 s TTL, key `(id, updated_at, reference)`) via
  `resolve_environment_secrets`. Errors and empty values → `Unavailable` with
  the error's `Display` (never the value or the reference).
- `invalidate_cache()` is called from `github_owner_tokens::{create,update,delete}`.
- FR-5 logging with per-owner last-source dedup.
- Testable inner fn with an injected resolver: selection, case-insensitive
  match, fallback for unknown owners, unavailable on resolve failure, cache hit
  within the TTL, miss after `updated_at` changes or `invalidate_cache`, no key
  access when nothing matched.
- Expose `github_owner_tokens::decrypt_rows_for(...)` as `pub(crate)` helpers
  as needed.

## Step 5: server routes and PR monitor

- `routes/workspaces/pr.rs`:
  - `create_pr` resolves creds for the repo, passes them to
    `check_remote_branch_exists`, `push_to_remote_if_needed` and
    `from_url_with_credentials`. `AuthFailed`/`CredentialUnavailable` → `PrError::GitCliNotLoggedIn`
    with the attributed message (`error_with_data_and_message`). `gh`
    `AuthFailed` → `CliNotLoggedIn` with `e.to_string()`.
  - `attach_existing_pr`, `get_pr_comments`, `create_workspace_from_pr`
    (`GhCli::with_credentials`), `resolve_pr_target` (credentials for all
    remotes, used for each candidate host).
  - `PrToolError::CliNotLoggedIn` gains `detail`, and the message uses it.
- `routes/workspaces/git.rs`: push, force push, branch status, rebase.
- `routes/repo.rs`: list open PRs (repo creds), PR info (URL creds).
- `pr_monitor.rs`: `resolve_for_urls(pool, [pr_url])`.
- `error.rs`: map `CredentialUnavailable` like `AuthFailed` if there is a
  match there.

## Step 6: UI copy and docs

- `GitHubOwnerTokensCard.tsx` description (FR-9).
- `docs/` page for GitHub organization tokens, if one exists: note that
  server-side operations use them.

## Step 7: Verify

- `cargo test -p utils -p git -p git-host -p services -p server` (targeted
  tests), `pnpm run check`, `pnpm run lint`, `pnpm run format`.
- No TS type changes are expected (`PrToolError` is not exported, `PrError`
  is unchanged). Run `pnpm run generate-types:check` to confirm.
