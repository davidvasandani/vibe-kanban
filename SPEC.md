# SPEC — Agents fall back to `gh` for PRs: MCP recovery ladder + PR-management tools

Task: `vk/53bc-agents-fall-back`

## Problem

Agents working in Vibe Kanban workspaces are told to open a PR through VK and
to merge it once CI is green. In `vk/91d8-upgrade-codex-cl` (homelab PR #1267)
the agent used `gh pr create` instead. It gave two reasons:

1. The harness said the VK MCP server had failed to connect —
   `vibe-kanban (CLIENT_HTTP_UNEXPECTED_CONTENT): "Unexpected content type: text/html"` —
   yet `mcp__vibe_kanban__*` tools worked in the same session.
2. The VK MCP has no PR tools, so "use VK's Create PR" could not be followed
   even with a healthy server.

The `gh` fallback also degraded. The workspace PAT has no Checks permission,
so `gh pr checks` (GraphQL `statusCheckRollup`) and `.../check-runs` both
returned 403, and CI state had to be guessed from `gh run list`.

## Root cause of the "failed to connect" banner (verified)

The banner names **`vibe-kanban`** (hyphen). The working server is
**`vibe_kanban`** (underscore). These are two different MCP entries:

| Entry | Source | URL | Result |
|---|---|---|---|
| `vibe_kanban` | user scope, written by VK into the scoped `~/.claude.json` | `http://127.0.0.1:18901/mcp` (deployment loopback route → Caddy adds the Cloudflare Access service token) | connected |
| `vibe-kanban` | project scope, the homelab repo's `.mcp.json` | `https://vibe.vasandani.dev/mcp` with `${VIBE_KANBAN_MCP_TOKEN}` / `${VIBE_KANBAN_CF_ACCESS_*}` headers | **failed** |

Agent sandboxes do not set those header variables, so the project entry calls
the public URL without credentials. Cloudflare Access answers with
`302` + `content-type: text/html` (its login redirect), which Claude Code
reports as `CLIENT_HTTP_UNEXPECTED_CONTENT`. Reproduced with `curl`, and
reproduced in Claude Code 2.1.281 by starting a session in a directory with
that `.mcp.json`:

- no extra settings → `vibe_kanban` (user) connected; `vibe-kanban` (project) failed;
- project entry renamed to `vibe_kanban` → the **project entry replaces the
  managed one** and the session gets 0 VK tools;
- `--settings '{"disabledMcpjsonServers":["<name>"]}'` → the project entry is
  dropped and the managed `vibe_kanban` connects with all tools, in both cases.

So the banner was accurate, but about a stale duplicate, not the server the
agent needed. Nothing in the harness itself had to "self-heal".

## Goals

### G1 — Stop shadowed duplicates of deployment-routed MCP servers (root cause)

When VK launches Claude Code, a project `.mcp.json` entry in the working
directory whose `url` / `httpUrl` is a URL that VK already serves through a
deployment runtime route (`VIBE_MCP_RUNTIME_ROUTES` — public URL key or
loopback value) is disabled for that session with
`--settings {"disabledMcpjsonServers":[…]}`. This applies whatever the
entry's name is. The VK-managed entry then wins, and the session no longer
shows a failed server that is really a duplicate.

- Only entries that exactly match a runtime route are disabled. Other project
  servers (`fetch`, `playwright`, `tldraw`, …) are untouched.
- If the profile already passes `--settings`, VK does not add a second one. It
  logs a warning and leaves the user's choice alone.
- No routes, no `.mcp.json`, or an unparseable `.mcp.json` → no flag, no error.

### G2 — An explicit recovery ladder before falling back

The VK MCP server's `instructions` (every mode) tell agents:

1. retry the failed call;
2. call `refresh_mcp_tools`;
3. call `restart_workspace` (it keeps the worktree, Git state, sessions and
   conversations);
4. only then fall back (e.g. `gh`), and say so in the final report, naming the
   rung reached.

The instructions also say that a harness "failed to connect" notice for a
differently named entry, e.g. `vibe-kanban` from a repository `.mcp.json`, is
not evidence this server is down. The test is a real call.

### G3 — Actionable upstream errors on VK's own hop

`send_json` / `send_empty_json` in the MCP server currently turn a non-2xx
into `VK API returned error status: 502` and drop the body. They also turn
`ApiResponse::error_with_data` into `Unknown error`. New behaviour:

- A non-2xx error includes the HTTP status, the `content-type`, and a bounded
  excerpt of the body (whitespace collapsed, ≤ 500 chars).
- A 2xx response that is not JSON (e.g. `text/html` from a proxy) reports
  `expected JSON, got <content-type>` plus the excerpt, not a serde error.
- An `error_data` payload is included in `details` when `message` is absent.
- The existing Create PR route answers with `error_with_data_and_message`, so
  MCP callers get a readable reason (e.g. target branch not found, `gh` not
  logged in).

### G4 — PR-management MCP tools

Five new tools, available in **global and orchestrator** modes. Each one is
scoped to a workspace (`workspace_id` defaults to the orchestrator context)
and to a workspace repo. `repo_id` is optional when the workspace has exactly
one repo. The PR is found from an optional `pr` argument (number or URL), or
else from the PR VK recorded for this workspace+repo, or else from a lookup by
the workspace branch.

| Tool | Contract |
|---|---|
| `create_pr` | `workspace_id?`, `repo_id?`, `title`, `body?`, `base?`, `draft?` → pushes the workspace branch (existing Create PR route) and opens the PR. Returns `{number, url, base, head_branch}`. |
| `get_pr` | → `{number, url, title, state, draft, merged, mergeable, merge_state, head_sha, head_branch, base_branch, merged_at, merge_commit_sha, checks: <summary>}`. |
| `list_pr_checks` | → `{head_sha, overall, complete, checks:[{name, source, status, conclusion, url}], sources:[{source, state, detail}]}`. |
| `merge_pr` | `method` (`squash`\|`merge`\|`rebase`, default `squash`), `delete_branch?` (default false), `force?` (default false) → refuses unless the gate below passes. Merges with a head-SHA guard. Returns `{merged, sha, message, branch_deleted, branch_delete_error}`. |
| `update_pr` | `title?`, `body?`, `ready_for_review?` (`true` → ready, `false` → back to draft). At least one field is required. |

#### Check status without the Checks permission

`list_pr_checks` reads three sources for the PR head SHA and reports how well
each one worked:

1. `GET repos/{o}/{r}/commits/{sha}/check-runs` (needs Checks: read);
2. `GET repos/{o}/{r}/commits/{sha}/status` (needs Commit statuses: read);
3. `GET repos/{o}/{r}/actions/runs?head_sha=…` + `…/runs/{id}/jobs` (needs
   Actions: read). This is the fallback when (1) is forbidden. Verified to
   work with the current fine-grained PAT, which gets 403 on (1) and (2).

Each source reports `ok` / `forbidden` / `error`. `complete` is true only
when check-runs **and** statuses were both readable. Otherwise the check list
may miss non-Actions checks, and the tool says so. `get_pr` also returns
GitHub's `mergeable_state` from the pulls endpoint. That value is GitHub's
own judgement across **all** required checks and reviews, and reading it
needs no Checks permission.

`overall` is `failing` if any check failed (`failure`, `timed_out`,
`cancelled`, `action_required`, `startup_failure`, status
`error`/`failure`). Otherwise it is `pending` if any check is not finished.
Otherwise it is `passing`, or `none` if no checks exist at all.

#### Merge gate (`merge_pr`)

These always refuse, even with `force`: PR not open, PR is a draft, or
`merge_state = dirty` (conflicts).

Without `force`, a merge also requires:
- `overall` ∈ {`passing`, `none`}. `failing` or `pending` → refused with the
  failing or pending check names.
- `merge_state` ∈ {`clean`, `has_hooks`}. `unknown` → refused as retryable
  ("GitHub is still computing mergeability"). `blocked`, `behind`,
  `unstable` → refused with the state.

When the gate passes, VK calls `PUT repos/{o}/{r}/pulls/{n}/merge` with
`sha = head_sha`, so a push that lands between the check and the merge makes
GitHub reject it (409) instead of merging unchecked code. Branch deletion
uses `DELETE repos/{o}/{r}/git/refs/heads/{branch}`. It is best effort,
reported separately, and never touches local checkouts. Merging does not
use `gh pr merge`, because it switches branches in the cwd.

#### Credentials

Every `gh` call runs from the repo's checkout directory. The deployment's
owner-routed `gh` wrapper then picks the org token for that remote, as the
existing Create PR does. A `pr` URL has to belong to the workspace repo's
remote (same owner/name, case-insensitive). Otherwise the tool refuses, so an
agent cannot use the workspace's credentials on an unrelated repository.
Azure DevOps and other providers return `unsupported_provider` for the new
operations.

## Non-goals

- Changing Claude Code's own connection banner, or retrying inside the
  harness. VK does not own that code. G1 removes the cause, and G2 tells the
  agent how to treat the notice.
- Editing the homelab repo's `.mcp.json`. It is still correct for humans
  running Claude Code with the variables set. VK neutralizes it in its own
  sessions only.
- Editing pipeline stage text, which is user data in the database.
- Reviews and approvals, PR comments (already exposed elsewhere), auto-merge
  queues, and rewriting the Create PR UI.
- Codex, Gemini and other executors. They do not read `.mcp.json`.

## Acceptance

1. A Claude Code session started by VK in a directory whose `.mcp.json`
   duplicates a runtime-routed VK URL has no failed duplicate entry. The
   managed server is connected (unit tests for detection and argument
   placement, plus the manual reproduction above).
2. The MCP instructions contain the four-rung ladder and the "report the rung
   / say you fell back" requirement (unit test).
3. A healthy session can open (`create_pr`), poll (`get_pr` /
   `list_pr_checks`) and merge (`merge_pr`) a PR entirely through MCP. Tools
   are registered in both modes (router test), and the gate, check
   aggregation, PR-reference parsing and repo-ownership checks are unit
   tested.
4. Check status is readable when check-runs return 403 (Actions fallback,
   `complete: false`, source coverage reported). Unit tested with a 403
   check-runs input.
5. A non-JSON or non-2xx backend response reaches the agent with status,
   content type and a body excerpt (unit test).
6. `cargo test -p mcp -p git-host -p executors -p server` for the touched
   code, `cargo clippy`, and `pnpm run format` all pass.
