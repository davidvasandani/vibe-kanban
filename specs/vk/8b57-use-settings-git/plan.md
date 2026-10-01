# Implementation Plan: Server-side GitHub operations use org tokens

**Spec**: `./spec.md`
**Status**: Draft
**Repo-root companions**: `SPEC.md` (FR detail), `IMPLEMENTATION_PLAN.md`

## Technical Context

- Rust 2024 workspace. The affected crates are `utils`, `git`, `git-host`,
  `services` and `server`, plus one string in `packages/web-core`.
- Storage: the existing SQLite table `github_owner_tokens`
  (`crates/db/src/models/github_owner_token.rs`). There is **no schema
  change**.
- Secrets: `McpGatewaySecretStore` envelopes
  (`services/github_owner_tokens.rs`), and `op://` resolution through
  `services/environment_secrets.rs::resolve_environment_secrets`.
- The server talks to GitHub through two subprocesses only:
  - `git` (`crates/git/src/cli.rs`, `GitCli::git_impl`)
  - `gh` (`crates/git-host/src/github/cli.rs`, `GhCli::run`), which `gh api`
    uses for REST and GraphQL.

  There is no in-process HTTP client for GitHub, so the "Authorization
  header" requirement is met by `GH_TOKEN`, which `gh` sends as
  `Authorization: token …`.

## Architecture & Approach

1. **A pure credential model in `utils`**
   (`crates/utils/src/github_credentials.rs`, new). It parses owner/repo from
   GitHub URLs, provides a `GitHubCredentials` set keyed by lowercase owner,
   and a `CredentialSelection` (`OrgToken` / `Unavailable` / `Fallback`) that
   knows how to:
   - apply itself to a git `Command`. It reuses
     `github_auth::routing_environment(owner, None, …)`, so the
     `GIT_CONFIG_PARAMETERS` text is identical to the agent path
     (constitution XXI). It also rewrites SSH URLs to HTTPS.
   - apply itself to a `gh` `Command` (`GH_TOKEN`, and removes competing
     token variables and `VK_GITHUB_ROUTED_OWNERS`).
   - attribute an auth-failure message (FR-8).

   `git` and `git-host` already depend on `utils`, so no new edges are added.
2. **git** (`crates/git/src/cli.rs`, `crates/git/src/lib.rs`).
   - Credentialed variants of `push`, `fetch_with_refspec` and
     `check_remote_branch_exists`, plus a new `remote_branch_oid` (ls-remote).
     The old names delegate with an empty set; workspace-manager's local-path
     mirroring uses them.
   - `GitService::{push_to_remote, check_remote_branch_exists,
     get_remote_branch_status, rebase_branch}` take `&GitHubCredentials`,
     which forces every server caller to supply them.
   - New `push_to_remote_if_needed` (FR-9).
   - `classify_cli_error` learns the 403 shapes.
   - New `GitCliError::CredentialUnavailable`.
3. **git-host** (`github/cli.rs`, `github/mod.rs`, `lib.rs`).
   - `GhCli` holds credentials, and `run` takes the target owner.
   - `GitHubProvider::with_credentials`.
   - `GitHostService::from_url_with_credentials`. `from_url` delegates with
     an empty set.
4. **The resolver in services** (`services/github_credentials.rs`, new).
   - `resolve_for_urls` / `resolve_for_repo`, each per request.
   - A 60 s TTL op cache keyed `(row id, updated_at, reference)`, which
     `github_owner_tokens::{create, update, delete}` invalidate.
   - Logging of the source used, deduplicated by last source per owner.
   - DB or decrypt failures make an owner `Unavailable` (fail closed). The key
     file is not touched when no requested owner has a row.
5. **Call sites**
   - `server/routes/workspaces/pr.rs`: `create_pr`, `attach_existing_pr`,
     `get_pr_comments`, `create_workspace_from_pr`, `resolve_pr_target`.
     `PrToolError::CliNotLoggedIn { detail }`.
   - `server/routes/workspaces/git.rs`: push, force push, branch status,
     rebase.
   - `server/routes/repo.rs`: list PRs, PR info.
   - `services/pr_monitor.rs`.
   - `server/error.rs`: maps `CredentialUnavailable` to 401 with its message.
6. **UI copy**: the description string in
   `packages/web-core/src/shared/dialogs/settings/settings/GitHubOwnerTokensCard.tsx`.

## Data Model

See `./data-model.md`. It covers in-memory types only. There are no
persistence changes.

## Contracts

See `./contracts/credential-interfaces.md`, which gives the Rust signatures of
the new public items and the changed ones.

## Research Notes

See `./research.md`.

## Constitution Check

- **XLIV, credentials follow the target resource.** Per-request, owner-keyed
  selection from Settings. Fallback happens only when no row exists, and is
  logged. Unreadable rows fail closed. Credentials are command-scoped
  (per-`Command` env), never global config. Errors are attributed to the
  source. ✔
- **XXI, one convention per concept.** Git helper text comes from
  `routing_environment`. `op://` detection and resolution reuse
  `normalize_secret_reference` and `resolve_environment_secrets`. The owner
  rule reuses `is_valid_owner` and `token_env_name`. ✔ Errors carry the owner
  and source, and never secrets. ✔
- **II, test the contract.** Unit tests in each crate, a real `git credential
  fill`, an `sh`-based env test for `gh`, and a local bare-remote push-skip
  test. ✔
- **III, small, reversible steps.** No schema or TS type change. Existing
  `from_url` and `GitCli` methods keep their behaviour for non-workspace
  callers. ✔
- **XVII/XXIII, secret redaction.** `GitHubToken` `Debug` is redacted.
  Selections never log values. ✔
- **XV, fail safe.** An `Unavailable` selection never spawns the command. ✔

No deviations.

## Risks & Dependencies

- `gh` must honour `GH_TOKEN` over the hosts.yml login. It does (documented
  precedence: `GH_TOKEN` > `GITHUB_TOKEN` > config).
- A homelab `githubAuth.orgTokenRefs` router on PATH would override
  `GH_TOKEN` for its owners. No host configures it, which is recorded in the
  spec's Out of Scope.
- Rewriting SSH to HTTPS changes the transport for configured owners. GitHub
  serves both for the same repository, and push-skip compares SHAs, not URLs.
- Changing `GitService` signatures touches the `crates/git/tests` call sites.
  Those are mechanical, with an empty credential set.
