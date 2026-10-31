# Implementation plan — vk/7561-investigate-vibe

See SPEC.md for the findings. The steps are in dependency order. VK steps
come first; the homelab steps depend on the VK release that introduces the
new env names and endpoint.

## A. Server-side 1Password credentials (R1)

1. `crates/services/src/services/environment_secrets.rs`
   - Add `pub fn server_credentials() -> Result<HashMap<String, String>, EnvironmentSecretError>`.
     - Read `VK_SERVER_OP_CONNECT_TOKEN_FILE`. It pairs with the ambient
       `OP_CONNECT_HOST`; a missing host returns `IncompleteConnect`.
     - Read `VK_SERVER_OP_SERVICE_ACCOUNT_TOKEN_FILE`.
     - Return them as `OP_CONNECT_HOST`, `OP_CONNECT_TOKEN` and
       `OP_SERVICE_ACCOUNT_TOKEN` entries.
   - File rules match `op-connect-token-read`:
     - one raw line;
     - one trailing `\n` allowed;
     - no NUL, no CR or LF inside, no surrounding whitespace, no `NAME=`
       assignment.
     - Any violation, or an unreadable file, returns a new variant,
       `CredentialFile { variable }`. Its message names the env var, never
       the path contents.
   - Split a pure `read_credential_file(path)` out for tests.
2. `crates/services/src/services/github_credentials.rs`. In `resolve_reference`,
   merge `server_credentials()?` into the single-reference map, so
   `select_credentials` sees deployment-supplied credentials as explicit
   ones: Connect first, then the service account. The ambient
   `OP_SERVICE_ACCOUNT_TOKEN` stays the last fallback.

## B. Owner-token health (R2)

3. `crates/services/src/services/github_credentials.rs`:
   - Add `pub async fn health(pool) -> GitHubOwnerTokenHealth`.
     - Resolve every row, through the same `resolve_row` and cache.
     - Then call `GET https://api.github.com/rate_limit` with a 10 s timeout,
       sending `Authorization: Bearer`, a `User-Agent`, and
       `X-GitHub-Api-Version`.
   - Classification:
     - unavailable: the resolve failed;
     - rejected: 401 or 403;
     - unavailable: a network error or another status (`detail` says
       "could not reach GitHub");
     - ok: 200.
   - Log state transitions per owner: `error` when an owner leaves ok,
     `info` when it recovers.
   - Write the classification as a pure function, `classify_probe(status)`,
     so it can be tested.
   - The types are `OwnerTokenHealth { owner, kind, status, detail }` and
     `GitHubOwnerTokenHealth { healthy, owners }`. They derive TS and are
     exported in `generate_types.rs`.
4. `crates/server/src/routes/github_owner_tokens.rs`: add
   `GET /github-owner-tokens/health`.

## C. `create_pr` head (R4)

5. `crates/server/src/routes/workspaces/pr.rs`
   - `CreatePrApiRequest` gains `#[serde(default)] #[ts(optional)] head: Option<String>`.
   - `PrError::InvalidHeadBranch { branch, reason }`.
   - A pure `validate_head(head, base, valid_name)` rejects:
     - empty names;
     - names `git2` considers invalid;
     - names with a remote prefix (`<remote>/`);
     - a head equal to the base.
     The handler also checks the local branch exists.
   - When `head` differs from the workspace branch, `head` is used at the push
     remote, the push and `head_branch`, and `PullRequest::create_for_workspace`
     plus the remote sync are skipped. A side PR is never recorded as the
     workspace's PR.
6. `crates/mcp/src/task_server/tools/pull_requests.rs`: add `head` to
   `CreatePrToolRequest`, add it to the payload, describe it, and return
   `head` plus `tracked: head.is_none()`.
7. Run `pnpm run generate-types`.

## D. Admin merge (R5)

8. `crates/git-host/src/types.rs`
   - `PrState.author: Option<String>`.
   - `pub enum MergeGate { Normal, Force, Admin }` replaces `force: bool`.
     `Admin` behaves like `Normal`, except that `blocked` is accepted.
   - Update the callers and tests.
9. `crates/git-host/src/github/cli.rs`: `RestPullRequest.user:
   Option<GhUserLogin>`, mapped to `author`.
10. A new `crates/services/src/services/admin_merge.rs`.
    - `AdminMergePolicy::from_env()` parses `VK_ADMIN_MERGE_ALLOW`.
      - Entries are separated by `;`, whitespace or newlines.
      - Each entry is `owner/repo:login[,login…]`, matched
        case-insensitively.
      - `*` is not allowed.
      - Malformed entries are logged and ignored.
    - `allows(repo, author) -> Result<(), String>` explains a refusal.
11. `pr.rs` `merge_pr`:
    - `MergePrApiRequest.admin` (serde default). `admin` together with
      `force` returns `InvalidPrReference`-style `MergeRefused`.
    - Run the policy check on `pr.base_repo` and `pr.author` before the gate.
    - Emit `tracing::warn!(target: "audit", …)` before the merge attempt and
      after it with the outcome.
    - Set `MergeOutcome.admin_bypass`. It is a serde field with a default of
      false.
12. MCP `merge_pr`: add `admin`, fix the `force` description (it does reach
    GitHub, which decides), and pass `admin` through.

## E. Ladder and errors (R6)

13. `crates/mcp/src/task_server/tools/mod.rs`
    - `ToolError` gains `kind: Option<String>`, taken from `error_data.type`.
    - A VK-explained error (envelope `message` or `error_data`) also gets
      `reported: true`.
    - `tool_error` writes `error_kind` and `vk_reported`.
14. `crates/mcp/src/task_server/tools/sessions.rs`
    - `default_session` picks the explicit session, else the orchestrator
      session, else the workspace's only session.
    - Several sessions return an error listing `name (id)`, so the agent can
      pass one. Used by `refresh_mcp_tools` and `restart_session`.
15. `crates/mcp/src/task_server/handler.rs`: rewrite `RECOVERY_LADDER`.
    - When to climb: transport and tool-discovery failures only.
    - When to skip: `vk_reported` errors, or the VK server unreachable.
    - Which IDs each rung needs.
    - The owner-routed token recipe for scripts.
    - Update the test.

## F. Docs (R7) and KB

16. `docs/integrations/mcp-server-configuration.mdx`: cover the `head`,
    `admin` and allowlist options, the health endpoint, server credential
    files, check visibility, and token recipes for scripts.
    `crates/mcp/AGENTS.md`: update the ladder section.

## G. Homelab (R3)

17. `modules/vibe-kanban.nix`: in the coordinator start scripts (prod and
    dev), export `VK_SERVER_OP_CONNECT_TOKEN_FILE` /
    `VK_SERVER_OP_SERVICE_ACCOUNT_TOKEN_FILE` pointing at
    `$CREDENTIALS_DIRECTORY/…`, when that credential is loaded.
18. Add a new `modules/vibe-kanban-github-token-alert.nix`.
    - A oneshot runs on a 30-min timer.
    - It curls `http://127.0.0.1:<port>/api/github-owner-tokens/health` and
      pages ntfy when `healthy` is false or the endpoint is unreachable.
    - A state file holds the cooldown: page on transition, plus re-page
      every 6 h while still failing. Recovery is posted once.
    - Enable it on think2 with the existing ntfy topic.
19. Run `nix-instantiate --parse` on the touched files, and a flake eval of
    think2 if feasible.

## Verification

- `cargo test -p services -p git-host -p mcp -p server`. Unset the
  host-sensitive env vars, as described in `wiki/host-env-sensitive-tests.md`.
- `pnpm run generate-types:check`, `pnpm run backend:check` and
  `pnpm run lint` (clippy) / `pnpm run format`.
