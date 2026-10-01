# SPEC: Server-side GitHub operations use the org-token table

Task: `vk/8b57-use-settings-git` ("Use Settings → GitHub organization tokens
for server-side git/PR operations (create_pr push, get_pr, merge_pr)").

## Problem

Settings → Repositories → **GitHub organization tokens** (`github_owner_tokens`
table, `services::github_owner_tokens`) stores one fine-grained PAT per GitHub
owner, either as a literal or an `op://` reference. Today those tokens reach
only **workspace agent processes**: `launch_environment` resolves them into
`VK_GITHUB_ROUTED_OWNERS` + `VK_GITHUB_PAT_<HEX>`, and `apply_github_routing`
turns those into owner-scoped `GIT_CONFIG_PARAMETERS` credential helpers and a
`gh` routing shim.

The VK server's own GitHub operations ignore the table:

- `git` network commands (`GitCli::push`, `fetch_with_refspec`,
  `check_remote_branch_exists`) inherit the server process's git config. In
  production that is the global `credential.https://github.com.helper = gh auth
  git-credential`, which uses whatever `GH_TOKEN`/`GITHUB_TOKEN`/`gh auth`
  login the server has.
- `gh` subprocesses (`git_host::github::GhCli::run`) inherit the server
  environment the same way.

Repro (2026-09-30, `sweetgreen/platform-ops`): the agent's own `git push`
worked with the `sweetgreen` org token. MCP `create_pr` failed three times with
`remote: Write access to repository not granted … 403`, even though the branch
was already pushed, because `create_pr` always pushes.

## Goals

1. Every server-side GitHub operation for a workspace repository authenticates
   with the org-token table entry for the **owner of the target repository**.
2. Server operations do not depend on the server's ambient GitHub credentials
   (`GH_TOKEN`, `GITHUB_TOKEN`, `gh auth`, host credential helpers), nor on
   their absence, whenever the owner has an org token.
3. Token edits apply on the next server call, with no restart.
4. `create_pr` does not push when the remote branch already equals local HEAD.
5. 401/403 failures name the owner and the credential source.

## Non-goals

- Changing how agent processes are routed (`apply_github_routing`, the shim).
- GitHub Enterprise hosts. Org tokens are scoped to `github.com`. Other hosts
  keep the existing server credential and are logged as fallback.
- Azure DevOps providers. They are unchanged.
- The homelab `githubAuth.orgTokenRefs` `gh` router. It is not configured in
  production. Where it is configured it still overrides `GH_TOKEN` for its own
  owners. This is recorded as a known interaction, not changed here.
- Shared-repository provisioning fetches (`workspace-manager`
  `SharedRepositoryStore::ensure` falling back to a forge fetch for a branch
  the checkout never fetched). That path is synchronous inside the workspace
  manager, has no database access, and is best-effort. It keeps the existing
  credential, and this is documented.

## Functional requirements

### FR-1 Owner parsing

`utils::github_credentials::parse_github_repo(url)` returns
`Some(GitHubRepoRef { owner, repo })` for `github.com` (and `www.github.com`)
URLs in these forms:

- `https://github.com/o/r`, `https://github.com/o/r.git`,
  `https://user[:pw]@github.com/o/r`, `http://…`, a trailing `/`.
- PR and tree URLs: `https://github.com/o/r/pull/12`.
- `ssh://git@github.com/o/r.git`, `ssh://git@github.com:22/o/r`.
- scp-like `git@github.com:o/r.git` and `github.com:o/r`.

The host comparison is case-insensitive. The owner must pass `is_valid_owner`.
Any other host or shape returns `None`, which means fallback. The owner keeps
its spelling. Lookups compare owners case-insensitively.

### FR-2 Credential resolution (per request)

`services::github_credentials::resolve_for_urls(pool, urls)` returns a
`GitHubCredentials` set. It covers every distinct `github.com` owner named by
`urls`, each entry being one of:

- `OrgToken(token)`: the owner has a row and its value decrypted (and
  `op://`-resolved) to a non-empty token.
- `Unavailable(reason)`: the owner has a row, but decryption, the 1Password
  lookup, or the key file failed, or the value resolved to empty. Operations
  on this owner **fail closed** with the reason. They never fall back.
- Owners with no row are absent, which means fallback.

The table is read from SQLite on **every** call, so edits apply immediately.
`op://` resolution is cached in-process with a 60 s TTL, keyed by
`(row id, updated_at, reference)`. The cache is also cleared on every
create/update/delete through the settings API, so re-saving the same reference
takes effect on the next call. Literal values are never cached beyond the
request. Resolved values and references are never logged, and never appear in
`Debug` output (`GitHubToken`'s `Debug` prints `<redacted>`).

The 1Password service-account token comes from the server's
`OP_SERVICE_ACCOUNT_TOKEN`, which is the existing `resolve_environment_secrets`
fallback.

### FR-3 Applying credentials to git subprocesses

`GitCli` network methods (`push`, `fetch_with_refspec`,
`check_remote_branch_exists`) accept `&GitHubCredentials` and select an entry
by the owner of the URL they contact. For `OrgToken`, that one command gets:

- `VK_GITHUB_PAT_<HEX>=<token>` in its environment;
- `GIT_CONFIG_PARAMETERS`, holding the existing value (if any) followed by
  `credential.https://github.com/<owner>.helper=` (a reset) and the inline
  helper that prints the variable (both the as-typed and the lowercase
  spelling). This is the same text `routing_environment` produces;
- `VK_GITHUB_ROUTED_OWNERS` removed, so a `gh` shim on the server's PATH
  cannot re-route;
- an SSH URL rewritten to `https://github.com/<owner>/<repo>.git` for that
  command, because SSH keys would otherwise bypass the helper.

Nothing is written to any git config file. For `Unavailable`, the command is
not run, and `GitCliError::CredentialUnavailable(message)` is returned. With no
entry, the command runs exactly as before (fallback).

### FR-4 Applying credentials to `gh` / REST / GraphQL

`GhCli` carries a `GitHubCredentials`. Every `gh` invocation names its target
owner (taken from `GitHubRepoInfo`, the remote URL, or the PR URL). For
`OrgToken`, the subprocess gets `GH_TOKEN=<token>`, and `GITHUB_TOKEN`,
`GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN` and `VK_GITHUB_ROUTED_OWNERS`
are removed. `gh` sends the token as the `Authorization` header for REST
(`gh api`) and GraphQL. The git parameters from FR-3 are added too, so `gh pr
checkout`'s fetch uses the same token. For `Unavailable`, `gh` is not run. With
no entry, the existing environment is inherited.

`GitHostService::from_url_with_credentials(url, credentials)` builds a
provider with credentials. `from_url` keeps its current behaviour, an empty
set, for non-workspace call sites.

### FR-5 Logging which source was used

Each owner a request resolves is logged once as
`owner=<owner> source=org-token|fallback|unavailable`, with no secret. This is
`info` when the owner's source changed since it was last logged and `debug`
otherwise, so the 60 s PR monitor does not flood the log.

### FR-6 Error attribution on 401/403

When a git or `gh` command fails with an authentication or permission error
(git: `Authentication failed`, `could not read Username`, `403`, `Write access
to repository not granted`, `Permission … denied`, `401`; gh: exit 4, `HTTP
401`, `HTTP 403`, `Bad credentials`, `Resource not accessible`), the error
message starts with an attribution:

- org token: `the sweetgreen org token (Settings → Repositories → GitHub
  organization tokens) was rejected or lacks access to sweetgreen/platform-ops`
- fallback: `no org token for owner sweetgreen; used the server's fallback
  credential, which was rejected or lacks access. Add a sweetgreen token in
  Settings → Repositories → GitHub organization tokens`

Then comes the original stderr. Unavailable reads `the sweetgreen org token is
configured but unavailable: <reason>`. These messages reach MCP callers:

- `create_pr` returns `PrError::GitCliNotLoggedIn` / `CliNotLoggedIn` with the
  attributed message, not the generic one.
- PR tool routes (`get_pr`, `list_pr_checks`, `merge_pr`, `update_pr`) map
  credential-attributed failures to `PrToolError::GithubError { detail }`
  carrying the attribution.

### FR-7 `create_pr` skips a redundant push

Before pushing, `create_pr` reads the push remote's `refs/heads/<branch>` with
`git ls-remote`, using the org-token credential. If that SHA equals the local
`refs/heads/<branch>` tip in the worktree, the push is skipped (logged at info)
and the remote-tracking ref and upstream are recorded locally, as a push would
have done. If the read fails or differs, the push runs as before. The
authoritative check is against the remote, not a possibly stale local
`origin/<branch>`.

### FR-8 Coverage

Credentials (FR-3/FR-4) are passed in at:

- `create_pr`: target-branch existence check, push-skip check, push, PR
  create (`gh repo view` for target and head, `gh pr create`).
- `attach_existing_pr`, PR comments, `create_workspace_from_pr`
  (`gh repo view`, `gh pr checkout`).
- `resolve_pr_target` and everything on `PrTarget`: `get_pr`/status, checks,
  merge, branch delete after merge, update, draft/ready, and the cross-remote
  `repo_identity` scan.
- `routes/workspaces/git.rs`: push, force push, branch status (fetch), rebase
  (fetch of the remote base).
- `routes/repo.rs`: list open PRs, PR info by URL.
- `pr_monitor`: `get_pr_status` for each open PR, by its PR URL owner.

### FR-9 Settings copy

The Org tokens card description changes "Changes apply to newly started
processes" to say that server operations (PR creation, status, merge, push)
use changes immediately, while running agent sessions pick them up on their
next start.

## Acceptance

- With only a `sweetgreen` org token and no `GH_TOKEN`/`GITHUB_TOKEN`/`gh
  auth` on the server, `create_pr` → `get_pr` → `merge_pr` works for a
  `sweetgreen/*` repo. Verified by unit tests that the `gh`/`git` subprocesses
  receive the org token and not the ambient one, and by the end-to-end PR for
  this task.
- With the token removed, the error says `no org token for owner sweetgreen`,
  not a bare 403.
- Rotating the token in Settings takes effect on the next call: the table is
  re-read and the cache is invalidated on write.
- Tests cover owner parsing (HTTPS, SSH, scp, PR URLs, case-insensitive
  owner/host, non-GitHub hosts), token selection, fallback, the unavailable
  fail-closed path, the env and GIT_CONFIG_PARAMETERS a git command receives
  (real `git credential fill`), gh env overrides, error attribution, the
  op-cache TTL and invalidation, and the skipped push.

## Risks

- `GIT_CONFIG_PARAMETERS` precedence: verified in the agent path. Command
  scope is read last, and the empty helper resets inherited helpers for that
  URL prefix only.
- Skipping the push also skips `push_to_remote`'s clean-worktree check.
  Uncommitted changes were never part of the PR, so opening it is correct.
