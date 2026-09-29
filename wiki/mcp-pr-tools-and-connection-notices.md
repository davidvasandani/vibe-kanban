# VK MCP PR tools, check coverage, and misleading connection notices

## A "failed to connect" notice names one config entry, not a service

`vk/91d8` fell back to `gh` because Claude Code reported
`vibe-kanban (CLIENT_HTTP_UNEXPECTED_CONTENT): "Unexpected content type: text/html"`
while `mcp__vibe_kanban__*` tools worked. Those are **two entries**:

- `vibe_kanban`: user scope, materialized by VK, pointing at the per-host
  loopback route (`VIBE_MCP_RUNTIME_ROUTES`, Caddy adds the Cloudflare Access
  token).
- `vibe-kanban`: homelab's `.mcp.json`, pointing at the public URL with
  `${VIBE_KANBAN_*}` headers that sandboxes never set. Cloudflare Access
  answers `302` + `text/html` (its login redirect).

The fastest diagnosis is the Claude `system/init` event. `mcp_servers[]`
carries `name`, `status` and `source` (`user`/`project`), so a failed
`project` entry next to a connected `user` entry is this bug. Reproduce with
`claude -p --output-format=stream-json --verbose … | grep '"subtype":"init"'`
from the repo directory.

Claude Code behaviours verified on 2.1.281 (the deployed pin):

- A project entry with the **same name** as a user entry *replaces* it, so
  the session silently loses every VK tool.
- `.mcp.json` is read from the launch directory **and every ancestor**, so a
  nested working directory still sees the repo root's file.
- `--settings '{"disabledMcpjsonServers":[…]}'` (inline JSON works) drops
  just those project entries and leaves the user entry connected.

VK now passes that flag for project entries on a runtime route, but only
when the launched agent's own `$HOME/.claude.json` has a managed entry on the
same route. Without that, the project entry might be the only connection.
The repository file is never edited, because humans with the variables set
still use it.

## Check status without the Checks permission

Fine-grained PATs here get **403** on `commits/{sha}/check-runs` and
`commits/{sha}/status` but can read `actions/runs?head_sha=` + `/jobs` and
`pulls/{n}`. GraphQL `statusCheckRollup` (what `gh pr checks` and
`gh pr view --json statusCheckRollup` use) fails the **whole** query, so never
request it in a query that also carries other fields.

Rules that made the aggregation trustworthy, each found in review:

- **Per-source coverage, not one verdict.** A 403 is a coverage fact
  (`forbidden`), not a call failure. `complete` requires check runs and
  statuses both read in full. A source whose `total_count` exceeds what was
  fetched is `truncated`, and a truncated list is never complete.
- **Actions jobs substitute; they never add.** Check runs already include
  Actions jobs, so jobs are read only when check runs are unreadable.
- **Drop superseded runs.** Keep the newest run per `(workflow_id, event)`.
  Otherwise an old failure on a re-dispatched commit reads `failing`
  forever.
- **Pass by allowlist.** A finished check passes only on `success`,
  `neutral` or `skipped`. `stale`, `action_required`, a missing conclusion,
  and any future value all fail, so an unfamiliar conclusion can never read
  as green.
- **Two witnesses to merge.** VK's check reading *and* GitHub's
  `mergeable_state ∈ {clean, has_hooks}` must agree, because the visible
  checks may be partial. `unknown` (just after a push) is retryable, not a
  refusal.

## Merging and editing safely through `gh`

- Merge with `PUT pulls/{n}/merge` and `sha = <evaluated head>`. A push that
  lands in between gets a 409 instead of an unchecked merge.
- Never use `gh pr merge --delete-branch`: it switches and deletes the
  *local* branch in the cwd. Delete the remote ref with
  `DELETE git/refs/heads/<branch>`, and **percent-encode** the branch name,
  since Git allows `#`, `%`, `?` and `feature#1` would address
  `heads/feature`.
- Delete only when the head repo equals the base repo. A fork PR's head
  branch lives in the fork, and deleting the same name in the base repo
  removes someone else's branch.
- Use `PATCH pulls/{n}` via `gh api --input <file>` for the title and body.
  `gh pr edit` also touches Projects fields. Draft ⇄ ready has no REST
  endpoint: use `gh pr ready [--undo] --repo`.
- Leave the local `pull_requests` row to the PR monitor after a merge. If it
  is marked merged directly, `get_open` drops the row, and the monitor never
  runs its post-merge handling (workspace archival).

## Which repository a PR reference means

- Resolve a URL, whether explicit or the recorded `pr_url`, against **all**
  of the checkout's remotes, and act through the one whose
  (host, owner, repo) matches. A PR opened with an `upstream/main` base lives
  upstream, and a bare number would hit the same-numbered PR on the default
  remote. Refuse URLs no remote owns, and compare the host too.
- `gh api` has no `--repo`. The deployment's owner-routed `gh` wrapper
  therefore routes `gh api` by its `repos/<owner>/<repo>/…` endpoint.
  Without that, calls against another owner's remote use the checkout
  default's token.

## Errors must say what came back

The MCP envelope decoder reports HTTP status, content type and a
≤ 500-char body excerpt for any non-2xx or non-JSON response, so a proxy's
HTML page is named as such. It also falls back to `error_data` when a route
sent no `message`. This builds on the rule in [[vk-pollers]]: any route an MCP
tool reaches must use `error_with_data_and_message`. Create PR also returns
provider refusals ("a pull request … already exists") as a message, not an
opaque 500, so agents act on them instead of reaching for `gh`.

## Contributed by

- `vk/53bc-agents-fall-back`
