# IMPLEMENTATION PLAN — vk/53bc-agents-fall-back

See `SPEC.md` for goals G1–G4 and acceptance. Each step leaves the tree
compiling, and tests land in the same step as the code they cover.

## Step 1 — G1: drop shadowed project MCP duplicates (executors)

`crates/executors/src/mcp_config.rs`
- `pub fn runtime_routed_mcp_url(url: &str) -> bool`: true when `url` is a
  key **or** a value of `VIBE_MCP_RUNTIME_ROUTES`. It is built on a pure
  `routes`-taking helper so tests do not touch process env.
- `pub fn shadowed_project_mcp_servers(mcp_json: &Value, is_routed: impl Fn(&str) -> bool) -> Vec<String>`:
  reads `mcpServers`, and for each object entry checks `url` / `httpUrl`.
  The result is sorted and deduplicated.

`crates/executors/src/executors/claude.rs`
- `fn project_mcp_suppression_args(args: &[String], current_dir: &Path) -> Option<[String; 2]>`:
  - `None` if `args` already contains `--settings` or `--settings=…`. A user
    profile override wins, and VK logs `warn!` if duplicates would have
    been disabled.
  - Reads `current_dir/.mcp.json`. On a missing file or bad JSON it returns
    `None` (unreadable files get a debug log).
  - Otherwise returns `["--settings", {"disabledMcpjsonServers":[…]}]` and
    logs `info!` with the names.
- Call it in `spawn_internal` after `into_resolved()` and append the pair to
  `args`. Every initial, follow-up and resume path goes through
  `spawn_internal`.
- Tests: detection (hyphen/underscore names, `httpUrl`, loopback value,
  unrelated URL untouched, malformed shapes), and argument construction
  (existing `--settings` respected, missing file, and the JSON shape of the
  flag).

## Step 2 — G3: actionable backend errors (mcp)

`crates/mcp/src/lib.rs`: `ApiResponseEnvelope` gains
`#[serde(default)] error_data: Option<serde_json::Value>`.

`crates/mcp/src/task_server/tools/mod.rs`
- `fn body_excerpt(bytes: &[u8]) -> String`: lossy UTF-8, whitespace
  collapsed, capped at 500 chars with `…`.
- `async fn read_api_envelope<T>(resp) -> Result<ApiResponseEnvelope<T>, ToolError>`:
  - on non-2xx: `VK API returned error status: {status}` with details
    `content-type: …; body: …`;
  - on 2xx that is not JSON (content-type present and not containing
    `json`): `VK API returned a non-JSON response (expected JSON, got …)`
    with the excerpt;
  - otherwise it parses from the bytes. A parse failure includes the excerpt.
- `send_json` / `send_empty_json` use it. When `success == false`, details
  are `message`, else the compact `error_data` JSON, else `Unknown error`.
- Tests: pure helpers (`body_excerpt`, the error formatter) over
  `(status, content_type, bytes)`.

`crates/server/src/routes/workspaces/pr.rs`
- `impl PrError { fn message(&self) -> String }`, and switch `create_pr`'s
  `error_with_data(…)` returns to `error_with_data_and_message(…, &e.message())`.

## Step 3 — G4 provider layer (git-host)

`crates/git-host/src/types.rs` gets new serde types (Serialize +
Deserialize, no ts-rs):
- `PrState { number, url, title, state: MergeStatus, draft, merged, mergeable: Option<bool>, merge_state: String, head_sha, head_branch, base_branch, merged_at, merge_commit_sha }`
- `CheckSource { CheckRuns, CommitStatuses, ActionsJobs }`, `SourceState { Ok, Forbidden, Error }`,
  `SourceCoverage { source, state, detail: Option<String> }`
- `PrCheck { name, source, status, conclusion: Option<String>, url: Option<String> }`
- `ChecksOverall { Passing, Failing, Pending, None }`
- `PrChecks { head_sha, overall, complete, checks, sources }` plus
  `pub fn aggregate_checks(head_sha, check_runs: SourceResult<Vec<PrCheck>>, statuses: …, actions: …) -> PrChecks`
  (a pure function; the Actions jobs only count when check-runs were not `Ok`,
  so nothing is counted twice).
- `MergeMethod { Squash, Merge, Rebase }`, `MergeOutcome { merged, sha, message, branch_deleted: Option<bool>, branch_delete_error: Option<String> }`
- `UpdatePrRequest { title, body, ready_for_review }`
- `pub fn merge_gate(pr: &PrState, checks: &PrChecks, force: bool) -> Result<(), MergeRefusal>`
  and `MergeRefusal { reason: String, retryable: bool }`, a pure function.
- `pub fn parse_pr_reference(input) -> Result<PrReference, GitHostError>`,
  where `PrReference = Number(i64) | Url { host, owner, repo, number }`.

`crates/git-host/src/github/cli.rs` (all with `dir = Some(repo_path)`, plus
`--hostname` for non-github.com hosts):
- `get_pr_state(repo_info, number, path)` → `gh api repos/o/r/pulls/n`
  (REST: `draft`, `merged`, `mergeable`, `mergeable_state`, `head.sha`, …).
- `list_check_runs(repo_info, sha, path)` → `…/commits/{sha}/check-runs?per_page=100`.
- `list_commit_statuses(repo_info, sha, path)` → `…/commits/{sha}/status`.
- `list_actions_jobs(repo_info, sha, path)` → `…/actions/runs?head_sha=…&per_page=50`,
  then `…/runs/{id}/jobs?per_page=100` for each run. A run with no jobs yet
  shows up as a pending entry named after the run.
- `merge_pr(repo_info, n, method, sha, path)` → `gh api -X PUT …/pulls/n/merge --input <tmp json>`.
- `delete_branch(repo_info, branch, path)` → `gh api -X DELETE …/git/refs/heads/{branch}`.
- `update_pr_fields(repo_info, n, title, body, path)` → `gh api -X PATCH …/pulls/n --input <tmp json>`.
- `set_ready(repo_info, n, ready, path)` → `gh pr ready n --repo spec [--undo]`.
- Parsers are pure `fn`s with fixture-JSON unit tests (check-runs, statuses,
  jobs, pulls, merge).

`crates/git-host/src/lib.rs` extends `GitHostProvider` with **default**
methods that return `UnsupportedProvider` (Azure stays untouched):
`get_pr_state`, `list_pr_checks`, `merge_pr`, `update_pr`, `repo_identity`.
`GitHubProvider` implements them. `list_pr_checks` fetches all three
sources, maps `InsufficientPermissions` → `Forbidden`, and calls
`aggregate_checks`.

## Step 4 — G4 backend routes (server)

In `crates/server/src/routes/workspaces/pr.rs`, nested under
`/api/workspaces/{id}/pull-requests`:
- `GET /status?repo_id&pr` → `PrStatusResponse { pr: PrState, checks: PrChecks }`
- `GET /checks?repo_id&pr` → `PrChecks`
- `POST /merge {repo_id, pr?, method?, delete_branch?, force?}` → `MergeOutcome`.
  It calls `merge_gate`; a refusal returns `error_with_data_and_message`.
  After a merge it updates the local `PullRequest` record status when one
  exists (`PullRequest::update_status`), best effort.
- `POST /update {repo_id, pr?, title?, body?, ready_for_review?}` → `PrState`
  (re-read after the update).

The shared resolver `resolve_pr_target(deployment, workspace, repo_id, pr) -> PrTarget { repo_path, remote_url, number, provider }`
works as follows:
1. Workspace repo row → `Repo` → the remote (target remote logic mirrors
   `create_pr`).
2. `pr` given → `parse_pr_reference`. A URL must match the remote's
   owner/repo (case-insensitive), else `pr_not_in_workspace_repo`.
3. `pr` absent → the newest `PullRequest` row for workspace+repo, else
   `list_prs_for_branch(workspace.branch)` preferring open. None →
   `no_pr_for_workspace`.

`PrError`-style typed errors all use `error_with_data_and_message`.

## Step 5 — G4 MCP tools (mcp)

New file `crates/mcp/src/task_server/tools/pull_requests.rs` with
`#[tool_router(router = pull_requests_tools_router)]`:
- A shared `resolve_pr_scope(workspace_id, repo_id)`: `resolve_workspace_id`
  → `scope_allows_workspace` → if `repo_id` is None,
  `GET /api/workspaces/{id}/repos` and require exactly one, else an error
  listing `name (id)`.
- `create_pr` → `POST /api/workspaces/{id}/pull-requests` (existing), then
  parse the number out of the returned URL.
- `get_pr`, `list_pr_checks`, `merge_pr`, `update_pr` → the new routes.
  Responses are passed through as `serde_json::Value`, so MCP needs no
  `git-host` dependency.
- Descriptions state the ladder hint for `merge_pr` ("poll `get_pr` until
  `checks.overall` is passing; `force` only on explicit instruction").
- Register the router in both `global_mode_router` and
  `orchestrator_mode_router`, and update the router test's expected set.

## Step 6 — G2 recovery ladder (mcp handler)

`crates/mcp/src/task_server/handler.rs`: add a `RECOVERY_LADDER` const
paragraph to the instructions in every mode, with a unit test asserting the
four rungs, their order, the `restart_workspace` preservation note, and the
"report the rung / say you fell back to gh" wording.

## Step 7 — Docs

- `crates/mcp/AGENTS.md`: a "Pull request tools" section covering sources,
  coverage, gate, head-SHA guard and credentials, plus a "Recovery ladder"
  section and a paragraph on "Shadowed project `.mcp.json` entries".
- `docs/integrations/vibe-kanban-mcp-server.mdx` (or the nearest existing MCP
  doc): list the new tools.

## Step 8 — Verify

`cargo test -p git-host -p mcp -p executors` (plus the server's `pr` route
tests if any), `cargo clippy --workspace --all-targets`, `pnpm run format`,
and a live `claude` reproduction of G1 with the built argument (already
recorded in SPEC). If the backend is not reachable from a dev build, the
end-to-end PR flow is covered by unit tests, and the report says the live
MCP PR flow is verified only after deploy.

## Risks

- `enum_dispatch` combined with default async-trait methods: confirm it
  compiles early in Step 3.
- `sqlx` offline cache: no new queries. Reuse the existing `PullRequest`
  functions only.
- `--settings` with inline JSON is handled by Claude Code ≥ 2.x (verified on
  2.1.281, the deployed pin).
