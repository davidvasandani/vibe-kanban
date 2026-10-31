# Make the VK PR tools usable for multi-owner workspaces

Task: vk/7561-investigate-vibe (follow-up to SWE-232, workspace `754c76c6`)

## Problem

During SWE-232 an agent used `gh` for nearly every PR action. The MCP
instructions told it to use `create_pr` / `get_pr` / `merge_pr`, and to climb
retry → `refresh_mcp_tools` → `restart_workspace` before any fallback. None of
those steps helped. Five items were reported. This spec records the root cause
of each one and what changes.

## Findings (root causes, verified 2026-10-31)

### 1. Org token unavailable (2026-10-02 to 2026-10-04)

- **Deterministic, not intermittent.** Since #352 (2026-10-01), the server's
  own git and `gh` calls resolve `op://` org-token rows with
  `resolve_environment_secrets`. The only credential that path can use is the
  server process's own `OP_SERVICE_ACCOUNT_TOKEN`. The coordinator unit
  (`homelab/modules/vibe-kanban.nix`, `vibe-kanban-dev` start script) runs
  `unset OP_SERVICE_ACCOUNT_TOKEN OP_CONNECT_TOKEN` before `exec`. Because
  think2 uses Connect, it never loads an `op-token` credential at all. The live
  process environment on think2 has only `OP_CONNECT_HOST`, with no token. So
  every `op://` row failed with `MissingToken` on every server-side call.
- **Why it "came back".** `GET /api/github-owner-tokens` shows that every row
  (alderbridge, davidvasandani, sweetgreen) was re-saved at
  2026-10-04T19:21Z and now carries no `reference`. They are literal PATs. The
  server's journal (from 2026-10-24) logs `source="org-token"` for sweetgreen.
  The "recovery" was a manual re-save of the rows as literals, not a transient
  1Password fault.
- **Why nothing alerted.** The error exists only in the HTTP response to the
  calling agent. Nothing checks configured owner tokens on its own: no health
  endpoint, no periodic probe, no page. A revoked or expired literal PAT would
  fail the same silent way.

### 2. `create_pr` only opens a PR from the workspace branch

`crates/server/src/routes/workspaces/pr.rs` hard-codes `workspace.branch` in
three places: the push remote, the push, and `head_branch`. The other PR tools
already accept an explicit `pr`, so only opening a PR is missing.

### 3. `merge_pr` cannot admin-merge

`merge_gate` refuses `merge_state = blocked`. `force` already skips that
refusal, but it also skips the failing-check, pending-check and `unknown`
gates. The tool text also says "GitHub branch protection still applies", which
reads as "this cannot help". The REST merge has no admin flag: a token whose
holder may bypass protection does so on a plain `PUT pulls/{n}/merge`, exactly
as `gh pr merge --admin` does. What is missing is a narrow, allowlisted and
audited bypass.

### 4. The fallback ladder does not match what a workspace agent can do

- The VK MCP is a shared HTTP server, so a workspace agent normally runs in
  Global mode with no context. `refresh_mcp_tools` and `restart_session`
  require `session_id` unless they run in Orchestrator mode, and the error
  tells the agent nothing it can act on.
- When the VK server itself is down (a Cloudflare 520), rungs 2 and 3 are VK
  tools too, so they cannot run.
- Rungs 2 and 3 repair tool discovery and transport. They cannot repair a
  configuration or authorization error that VK reported with a message, such
  as item 1.

### 5. Check visibility

Checked live on terraform-infrastructure #1826:

- `list_pr_checks` returns every `atlantis/*` and Devin commit status
  (`commit_statuses: ok`) and the Actions jobs (`actions_jobs: ok`).
- `check_runs` is `forbidden`. Fine-grained PATs cannot be granted the Checks
  permission (it is GitHub-App-only), so check runs from non-Actions GitHub
  Apps stay invisible. `complete: false` says so.

No code fix is possible with PATs. This is documented.

### 6. Ambient `GH_TOKEN` / `GITHUB_TOKEN` in workspaces

The ambient variable is a personal token. The owner-routed token is reachable
without seeing it: `GH_REPO=<owner>/<repo> gh auth token` runs through the VK
`gh` shim, and `git credential fill` uses the owner-scoped helper. Both
returned the same routed credential in a live check. This is documented, and
VK does not export another token-shaped variable.

## Requirements

- **R1 (item 1, fix).** The server can resolve `op://` org-token rows without
  holding a token in its environment. It reads 1Password credentials from
  files named by `VK_SERVER_OP_CONNECT_TOKEN_FILE` (with the ambient
  `OP_CONNECT_HOST`) and `VK_SERVER_OP_SERVICE_ACCOUNT_TOKEN_FILE`, at call
  time. Connect is tried first, the service account is the fallback, and the
  ambient `OP_SERVICE_ACCOUNT_TOKEN` stays the last fallback. Agent launches
  keep their org-scoped rules unchanged.
- **R2 (item 1, health).** `GET /api/github-owner-tokens/health` reports, for
  each row, `owner`, `kind` (`literal`/`reference`), `status` (`ok`,
  `unavailable` or `rejected`) and a secret-free `detail`. `ok` means the token
  resolved and GitHub accepted it (`GET /rate_limit`, which does not count
  against the quota). The overall `healthy` flag is true when every row is
  `ok`. A transition to non-ok is logged at `error`.
- **R3 (item 1, alert).** Homelab passes the credential file paths to the
  coordinator. A timer on the coordinator polls the health endpoint and pages
  ntfy when it is unhealthy or unreachable, at most once per cooldown.
- **R4 (item 2).** `create_pr` accepts an optional `head`. When it is set, the
  branch must be a valid local branch name, must not be remote-prefixed, and
  must differ from the base. It is pushed and used as the PR head.
  - A side PR is **not** recorded as the workspace's PR. Default PR selection,
    the PR badge and post-merge workspace archival stay tied to the workspace
    branch.
  - The response says `recorded: false`. Follow-up calls pass `pr` explicitly.
- **R5 (item 3).** `merge_pr` accepts `admin: true`. It is allowed only when
  the PR's `owner/repo` and author match `VK_ADMIN_MERGE_ALLOW`, a deployment
  environment variable. It is deliberately not a Settings field, so an agent
  cannot grant itself the bypass through the VK API.
  - The admin gate refuses everything the normal gate refuses, except
    `blocked`.
  - Every admin attempt and its outcome are logged under the `audit` tracing
    target, and the response carries `admin_bypass: true`.
  - `admin` and `force` together are rejected.
- **R6 (item 4).** Rewrite the ladder in the MCP instructions:
  - Rungs 2 and 3 are for transport and tool-discovery failures only.
  - An error VK returned with an explanation (authentication, configuration,
    a refusal, the provider's message) goes straight to fixing the cause or
    falling back.
  - When the VK server is unreachable, skip to the fallback.
  - Name the IDs the recovery tools need.
  - `refresh_mcp_tools` and `restart_session` default `session_id` when the
    workspace has exactly one session, and otherwise list the candidates.
  - MCP error JSON passes the typed `error_data.type` through as `error_kind`.
- **R7 (items 5, 6).** Docs and tool descriptions say:
  - non-Actions check runs are invisible with PATs, and statuses such as
    Atlantis are visible;
  - scripts get an owner-routed token through `GH_REPO=… gh auth token` or
    `git credential fill`, never the ambient `GH_TOKEN`/`GITHUB_TOKEN`.

## Non-goals

- Making check runs visible. That would need a GitHub App.
- Changing how agent launches resolve org Env Vars.
- Re-converting the current literal rows to references.

## Acceptance

- Unit tests:
  - credential-file selection (Connect first, file errors fail closed, no
    value is ever echoed);
  - the health classification;
  - head-branch validation;
  - the admin allowlist parse and match;
  - the admin gate (`blocked` allowed; failing, pending, dirty, draft and
    unknown still refused);
  - the session default;
  - `error_kind` pass-through;
  - the ladder text.
- `pnpm run generate-types:check`, `cargo test` for the touched crates, and
  clippy/format all pass.
- The homelab module evaluates (`nix-instantiate --parse` / flake check, where
  available).
