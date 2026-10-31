# Prior knowledge — vk/7561-investigate-vibe

Searched `vibe-kanban/wiki`, `homelab/knowledge-base`, `homelab/docs/knowledge`
and `homelab/.specify/memory/constitution.md` for 1Password, org tokens, PR
tools, the recovery ladder, check coverage, ntfy and alerting.

## Directly relevant pages

- **wiki/1password-reference-resolution.md** (`vk/76ab`)
  - `op://` org Env Vars resolve per launch, and the org's literal
    `OP_SERVICE_ACCOUNT_TOKEN` wins over the ambient one.
  - Connect is **org-only, never ambient**. The coordinator exports
    `OP_CONNECT_HOST` but unsets `OP_CONNECT_TOKEN` (security decision,
    option A).
  - Failures are classified and stderr is never echoed.
  - The service account has an account-wide quota of 1,000 requests per
    24 h.
  - The server-side git/`gh` path "uses the server's token". This task found
    that the coordinator has none (see SPEC.md).
- **wiki/github-owner-token-routing.md** (`vk/0f52`, `vk/8b57`)
  - One PAT per owner, stored in the encrypted `github_owner_tokens` table.
  - Server-side operations resolve a per-request `GitHubCredentials` set. The
    `op://` cache lasts 60 s and Settings writes invalidate it.
  - Unavailable owners fail closed, and 401/403 errors are attributed.
  - The `gh` shim routes by `-R`, the API endpoint, `GH_REPO` and the cwd
    repo.
  - Never `Debug` a `Command`, because it prints environment changes.
- **wiki/mcp-pr-tools-and-connection-notices.md** (`vk/53bc`)
  - Fine-grained PATs get 403 on check-runs. Statuses and Actions jobs are
    readable.
  - Coverage is reported per source, and only an allowlist of conclusions
    counts as passing.
  - Merging needs two witnesses: VK's checks plus `mergeable_state` of
    clean or has_hooks.
  - The merge is SHA-guarded.
  - Leave the `pull_requests` row to the PR monitor, which runs post-merge
    archival.
  - Error envelopes carry status, content type and an excerpt.
- **wiki/host-env-sensitive-tests.md**: the worker GitHub-routing test fails
  on agent hosts because of real `VK_GITHUB_PAT_*` and `GIT_CONFIG_*`. Re-run
  it with those unset.
- **homelab constitution principle 143**
  - Prove a tool is unusable before routing around it.
  - The ladder must stay honest, and instructions must name tools that exist.
  - Partial check coverage is never shown as all-green.
- **homelab knowledge-base/onepassword-rate-limits-from-agent-sessions.md**
  - `op-connect-token-read` accepts one raw line, rejects assignment syntax,
    and strips at most one trailing newline.
  - Its stdin is the program itself, so pass the value on another
    descriptor.
- **homelab modules/vibe-kanban-slack-probe-alert.nix**: the existing ntfy
  paging pattern (cooldown, `curl -fsS -m 10`, Title, Priority and Tags
  headers). Constitution VII says signals must page, not sit in journald.

## Consequences for this task

- Any server-side 1Password credential must come from the deployment
  explicitly, as files, so that the env-unset guard (no `OP_*` token
  inherited by agents) stays intact.
- A health probe must not spend the service-account quota carelessly. Use
  Connect where it is configured, and probe at most every 30 min.
- An admin-merge allowlist must not be editable through the VK API that
  agents can reach.
- A side PR must not become the workspace's recorded PR. If it did, it would
  change default PR targeting and post-merge archival.
