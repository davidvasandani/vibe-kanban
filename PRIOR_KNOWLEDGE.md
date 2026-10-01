# Prior knowledge: server-side GitHub operations and org tokens

Task: `vk/8b57-use-settings-git`. This file distills what the project
knowledge bases (`wiki/` and `docs/knowledge-base/`) already record about this
problem area. The knowledge bases were only read in this stage, not changed.

## Relevant pages

- `wiki/github-owner-token-routing.md` (`vk/0f52-manage-gh-token`)
- `wiki/mcp-pr-tools-and-connection-notices.md` (`vk/53bc-agents-fall-back`)
- `docs/knowledge-base/workspace-environment-inheritance.md` (several tasks,
  including `vk/0f52`)
- `wiki/vk-pollers.md` (rule: MCP-reachable routes must carry a message)

## What to build on

### Storage and secrecy (from `github-owner-token-routing`)

- Tokens live in their own SQLite table `github_owner_tokens`, never in
  `Config`. `/api/info` returns the whole Config, and `PUT /api/config`
  overwrites it.
- Owner uniqueness is `UNIQUE COLLATE NOCASE`, so lookups by owner should be
  case-insensitive.
- Values are `McpGatewaySecretStore` envelopes under a separate host key,
  bound by AAD to the row id. Only `op://` references are ever returned. The
  empty-table path never touches the key file. Keep that property: a server
  with no org tokens must not create or read the key on every PR poll.
- Runtime `sqlx::query_as` calls keep the `.sqlx` offline cache unchanged.

### Git credential mechanics (verified with real git)

- `credential.https://github.com/<owner>.helper=` (an empty value, which
  resets inherited helpers) followed by an inline `!f(){…}` helper routes only
  that owner's URLs. Command-scope config (`GIT_CONFIG_PARAMETERS`) is read
  last.
- Path matching is case-sensitive, so emit both the as-typed and the
  lowercase spelling.
- Use `GIT_CONFIG_PARAMETERS` (sq-quoted, appended after any existing value),
  not `GIT_CONFIG_COUNT`/`KEY_n`.
- The helper names an env var (`VK_GITHUB_PAT_<HEX>`), so the token never
  appears in config text. `routing_environment(owners, None, lookup)` already
  produces exactly this text without the shim.
- Testing recipe: `git credential fill` under a temporary `HOME` with
  `GIT_CONFIG_NOSYSTEM=1`; a fake `gh` that prints `GH_TOKEN`.

### Fail-closed semantics

- A configured owner whose token is empty or unresolvable fails closed and
  names the owner. Unconfigured owners pass through unchanged. That matches
  this task's "fallback only when no configured token".
- `environment_secrets`: the literal `OP_SERVICE_ACCOUNT_TOKEN` in org Env Vars
  wins over the service env. Ambient `OP_*` variables are stripped. Reads are
  bounded to 30 s. Errors never carry provider output or values.

### PR tools (from `mcp-pr-tools-and-connection-notices`)

- Fine-grained PATs get 403 on check-runs and status. That is a coverage fact
  (`SourceRead::Forbidden`), not a call failure. Credential attribution must
  not turn those per-source 403s into hard failures of `list_pr_checks`.
- Merge is `PUT pulls/{n}/merge` with a `sha` guard. The branch is deleted via
  `DELETE git/refs/heads/<enc>` only when the head repo equals the base repo.
- A PR URL is resolved against **all** of the checkout's remotes, so
  credentials must cover every remote's owner, not just the default remote's.
- `gh api` has no `--repo`. The owner is known from `GitHubRepoInfo` in
  `GhCli::api_args`, so it can be passed explicitly.
- Every MCP-reachable route error must carry a `message`
  (`error_with_data_and_message`). Provider refusals become messages, not 500s.

### Deployment interaction (from `workspace-environment-inheritance`)

- The Nix `gh` router (`githubAuth.orgTokenRefs`) still exists, but no host
  configures it. Where configured, it overrides `GH_TOKEN` for its own
  owners. That is out of scope here.
- Keep the long-lived server process environment secret-free. Inject secrets
  only into the specific child process. Never mutate the server env or write
  git config.

## Gaps the knowledge base does not cover (new in this task)

- Nothing yet describes server-side (non-agent) credential selection, op
  resolution caching, or attributing auth errors to a credential source.
