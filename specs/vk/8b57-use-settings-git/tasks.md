# Tasks: Server-side GitHub operations use org tokens

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Credential model (foundation)
- [x] T001 Create `crates/utils/src/github_credentials.rs`: `GitHubRepoRef`,
  `parse_github_repo`, `GitHubToken` (redacted Debug), `OwnerCredential`,
  `GitHubCredentials`, `CredentialSelection` (`apply_git`, `apply_gh`,
  `attribute`, `source_label`), and the auth-failure detectors. Export it from
  `crates/utils/src/lib.rs`.
- [x] T002 Unit tests in `crates/utils/src/github_credentials.rs`: the parse
  table, case-insensitive selection, fallback, SSH rewrite, a real `git
  credential fill` with an ambient helper, `gh` env override via `sh`,
  attribution wording, redaction (depends on T001).

## Phase 2: Consumers of the model
- [x] T003 [P] `crates/git/src/cli.rs`: `CredentialUnavailable`, 403
  classification, `push_with` / `fetch_with_refspec_with` /
  `check_remote_branch_exists_with` / `remote_branch_oid`, attribution on
  auth failure (depends on T001).
- [x] T004 [P] `crates/git-host/src/github/cli.rs`: credentials on `GhCli`,
  `run(owner, …)` applying the selection, attribution, owner passed at every
  call (depends on T001).
- [x] T005 `crates/git/src/lib.rs`: credential parameters on `push_to_remote`,
  `check_remote_branch_exists`, `get_remote_branch_status`, `rebase_branch`
  and the fetch helpers. Add `PushOutcome` and `push_to_remote_if_needed`
  (depends on T003).
- [x] T006 `crates/git-host/src/github/mod.rs`, `crates/git-host/src/lib.rs`:
  `GitHubProvider::with_credentials`,
  `GitHostService::from_url_with_credentials` (depends on T004).
- [x] T007 [P] Update `crates/git/tests/*.rs` call sites for the new
  signatures. Add push-skip tests (skip when equal, push when different)
  (depends on T005).

## Phase 3: Resolver
- [x] T008 Create `crates/services/src/services/github_credentials.rs`
  (resolve_for_urls / resolve_for_repo, op cache with TTL + invalidation,
  deduplicated source logging, fail-closed). Register it in
  `crates/services/src/services/mod.rs` (depends on T001, T005).
- [x] T009 `crates/services/src/services/github_owner_tokens.rs`: call
  `github_credentials::invalidate_cache()` on create/update/delete and expose
  the decrypt helper `pub(crate)` (depends on T008).
- [x] T010 Resolver unit tests in
  `crates/services/src/services/github_credentials.rs`: selection,
  case-insensitive match, unknown owner → fallback, resolve failure →
  unavailable, cache hit, miss after `updated_at` change or invalidation, no
  key access without a match (depends on T008, T009).

## Phase 4: Call sites
- [x] T011 `crates/server/src/routes/workspaces/pr.rs`: `create_pr` (creds,
  push-skip, attributed errors), `attach_existing_pr`, `get_pr_comments`,
  `create_workspace_from_pr`, `resolve_pr_target`,
  `PrToolError::CliNotLoggedIn { detail }` (depends on T005, T006, T008).
- [x] T012 [P] `crates/server/src/routes/workspaces/git.rs`: push, force push,
  branch status, rebase (depends on T005, T008).
- [x] T013 [P] `crates/server/src/routes/repo.rs`: list open PRs, PR info
  (depends on T006, T008).
- [x] T014 [P] `crates/services/src/services/pr_monitor.rs`: credentials per
  PR URL (depends on T006, T008).
- [x] T015 [P] `crates/server/src/error.rs`: map `CredentialUnavailable` to
  401 with its message (depends on T003).
- [x] T016 Fix any other compile breaks from the signature changes (search
  `crates/` for `push_to_remote`, `rebase_branch`, `get_remote_branch_status`,
  `check_remote_branch_exists`) (depends on T011–T015).

## Phase 5: UI, docs, validation
- [x] T017 [P] Settings copy in
  `packages/web-core/src/shared/dialogs/settings/settings/GitHubOwnerTokensCard.tsx`.
- [x] T018 [P] Docs: note server-side use in the docs page that describes
  GitHub organization tokens, if one exists (`docs/**`). (No such page exists, so nothing to change; the Settings card copy is the user-facing description.)
- [x] T019 Validation: `cargo test` for utils, git, git-host, services and
  server. Run `cargo clippy`, `pnpm run generate-types:check`,
  `pnpm run check`, `pnpm run lint` and `pnpm run format` (depends on all of
  the above).
