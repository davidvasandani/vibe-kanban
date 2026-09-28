# PRIOR_KNOWLEDGE — vk/53bc-agents-fall-back

Sources searched: `vibe-kanban/wiki/` (the VK project knowledge base,
`INDEX.md` + 38 pages), `homelab/knowledge-base/` (index + MCP/Cloudflare
pages), prior spec folders `homelab/specs/vk/{733b-vk-mcp-error,ffeb-debug-vk-mcp-err}`,
and `vibe-kanban/crates/mcp/AGENTS.md`. The knowledge base was read only.

## Directly applicable

1. **Every route an MCP tool can reach must use `error_with_data_and_message`**
   (`wiki/vk-pollers.md`, "Assert what the caller receives"). `ApiResponse::error_with_data`
   sets `message: None`. The MCP envelope only reads `message`, so agents
   see `{"error":"VK API returned error","details":"Unknown error"}`. The
   existing Create PR route (`routes/workspaces/pr.rs::create_pr`) still uses
   the message-less form for every `PrError`. That is a trap for a new
   `create_pr` MCP tool, and the lesson is to test what crosses the boundary,
   not the enum variant.
2. **MCP self-healing that already exists** (`crates/mcp/AGENTS.md`,
   "Resilience"): atomic port-file writes, a retrying port-file read, and
   `send_with_reconnect` (re-resolve the backend URL and retry once on
   connect/timeout errors). That covers VK's stdio→backend hop only. It does
   nothing for the harness→HTTP-gateway hop, where the reported banner came
   from.
3. **Recovery tools** (`crates/mcp/AGENTS.md`, "Recovering a wedged MCP
   session"): `restart_session` is cheap; `restart_workspace` stops workspace
   processes and cold-starts the session but preserves the worktree, Git state,
   sessions and conversations. `refresh_mcp_tools` works for Codex; for Claude
   Code it returns `unsupported` with `restart_session` as remediation.
   `ClaudeMcpInventory` already reads `system/init.mcp_servers` statuses. The
   ladder must therefore allow "refresh returned unsupported → go to the next
   rung".
4. **Deployment routing of the VK MCP URL** (`crates/executors/src/mcp_config.rs`,
   homelab `modules/vibe-kanban-rebuild.nix`): settings keep the *public*
   URL `https://vibe.vasandani.dev/mcp`. `VIBE_MCP_RUNTIME_ROUTES` maps it to a
   per-host loopback Caddy (`127.0.0.1:18901/mcp`) that injects the Cloudflare
   Access service token and converts Access redirects/HTML to JSON-RPC errors.
   `has_runtime_route_for_public_url` already exists. The variable is present
   in the agent environment.
5. **Cloudflare Access returns HTML to unauthenticated clients** (homelab
   `knowledge-base/cloudflare-access-service-token-live-enablement.md`,
   `edge-safe-error-statuses.md`): without a service token the edge answers
   `302` → login page (`text/html`). That is why the loopback gateway exists
   and why a direct public-URL entry fails with
   `CLIENT_HTTP_UNEXPECTED_CONTENT`.
6. **Owner-routed `gh`** (homelab `vibe-kanban-rebuild.nix`, `githubGhRouter`):
   the deployment's `gh` wrapper picks an org token from `--repo`/`-R` or,
   failing that, from the cwd's git remote. `gh api` has no `--repo` flag, so
   new `gh api` calls must run with `cwd` = the repo checkout or they fall back
   to the default token. The existing `get_pr_review_comments` runs with no
   cwd, which is a latent gap and is not changed here.

## Verified in this task (new evidence, not yet in the KB)

- The banner's server `vibe-kanban` (hyphen) comes from homelab's `.mcp.json`
  (project scope, `${VIBE_KANBAN_*}` headers that are unset in sandboxes). It
  is a different server from the VK-managed `vibe_kanban`.
- Claude Code 2.1.281: a project `.mcp.json` entry with the **same name** as a
  user entry replaces it. That can silently remove every VK tool.
  `--settings '{"disabledMcpjsonServers":[name]}'` drops the project entry and
  lets the user entry connect.
- Fine-grained PAT capability, probed on homelab: `commits/{sha}/check-runs`
  → 403, `commits/{sha}/status` → 403, `actions/runs?head_sha=` + `/jobs` → OK,
  `pulls/{n}` → OK, with `mergeable_state` (e.g. `blocked`) present.
  `gh pr view --json statusCheckRollup` fails the whole query, so never
  request it.

## Constraints carried forward

- Do not hand-edit `shared/types.ts`. The new server-only/MCP types do not
  need ts-rs export.
- Tool registration is pinned by `orchestrator_mode_exposes_only_scoped_workflow_tools`.
  Adding tools to orchestrator mode means updating that expected set.
- `stop_*` tools skip workspace scope checks on purpose. The new PR tools are
  workspace-scoped and must call `scope_allows_workspace`.
- Wiki conventions: one topic per page, a `## Contributed by` footer with task
  ids, and a one-line entry in `wiki/INDEX.md`.
