# Feature Specification: Server-side GitHub operations use org tokens

**Feature dir**: `specs/vk/8b57-use-settings-git/`
**Status**: Draft
**Task**: `vk/8b57-use-settings-git` (technical detail in the repo-root `SPEC.md`)

## Summary

Settings → Repositories → GitHub organization tokens holds one token per
GitHub owner. Agent sessions already use them. The Vibe Kanban server's own
GitHub work does not: pushing and opening a PR for `create_pr`, reading,
checking, merging and editing PRs, and background PR polling all use whatever
credential the server process happens to have. A repository whose org token
can write therefore still fails `create_pr` with a bare 403, and an agent has
no way to tell which credential to fix. This feature makes every server-side
GitHub operation authenticate with the target repository owner's org token. It
makes errors name the credential that was used, and stops `create_pr` from
pushing a branch that is already up to date.

## User Stories

- As an operator, I want to configure a GitHub owner's token once in Settings,
  so that both agents and the VK server use it for that owner's repositories.
- As an agent using the VK MCP PR tools, I want `create_pr`, `get_pr`,
  `list_pr_checks`, `merge_pr` and `update_pr` to work for a repository the
  configured token can access, so that I never need a `gh` fallback.
- As an operator rotating a token, I want the new value used on the next
  operation without restarting VK.
- As an operator diagnosing a failure, I want the error to say which owner and
  which credential source (org token or server fallback) was rejected, so that
  I know which token to fix.
- As an agent that already pushed its branch, I want `create_pr` to open the
  PR without pushing again, so that a server push problem cannot block it.

## Functional Requirements

- FR-1: For each server-side GitHub operation, the system determines the owner
  of the target repository from its remote or PR URL. It accepts HTTPS, SSH
  and scp-style `github.com` remotes, and matches the owner
  case-insensitively.
- FR-2: If the owner has an org token, the operation authenticates with that
  token and nothing else. The server's ambient credentials (environment
  tokens, CLI login, host credential helpers) have no effect on it, whether
  present or absent.
- FR-3: If the owner has no org token, the operation uses the existing server
  credential, unchanged (fallback).
- FR-4: If the owner has an org token that cannot be read or resolved, the
  operation fails with a message naming the owner. It never falls back to
  another credential.
- FR-5: The org-token configuration is read for every operation. `op://`
  references are resolved at call time, may be cached briefly, and are never
  logged. A Settings create/update/delete affects the next operation.
- FR-6: Credentials are applied only to the single command or request that
  needs them. They are never written to persistent configuration or to the
  server's environment.
- FR-7: The system logs which credential source each owner used (org token,
  fallback or unavailable), without secrets, and does not flood the log from
  background polling.
- FR-8: Authentication and permission failures (401/403) produce a message
  that names the owner and the credential source. For example: "the
  sweetgreen org token … was rejected or lacks access", or "no org token for
  owner sweetgreen; used the server's fallback credential …". Per-source
  check-read 403s stay coverage facts, as they are today.
- FR-9: `create_pr` skips the push when the remote branch already points at
  the local branch's commit. Otherwise it pushes as before.
- FR-10: Coverage includes `create_pr` (target-branch check, push, PR
  creation), `get_pr`/status, `list_pr_checks`, `merge_pr` (including branch
  deletion), `update_pr`, PR comments, attaching an existing PR, creating a
  workspace from a PR, workspace branch push/force push, branch status fetches,
  rebase fetches, repository PR listing, and the background PR monitor.
- FR-11: The Settings card text states that server operations use token
  changes immediately, while running agent sessions use them on their next
  start.

## Out of Scope

- How agent sessions are routed. That path already uses org tokens.
- GitHub Enterprise hosts and Azure DevOps. They keep the existing credential.
- The optional Nix-level `gh` router (`githubAuth.orgTokenRefs`). No host
  configures it.
- Best-effort shared-repository provisioning fetches inside the workspace
  manager. They have no access to settings and are already allowed to fail.

## Acceptance Criteria

- [ ] With only a `sweetgreen` org token and no ambient GitHub credential,
      `create_pr`, `get_pr` and `merge_pr` succeed for a `sweetgreen/*` repo.
      Unit tests show that the git and `gh` children receive the org token
      and not an ambient one. The end-to-end proof is this task's own PR.
- [ ] With the token removed, a failing call reports "no org token for owner
      sweetgreen", not a bare 403.
- [ ] Editing the token in Settings changes the token used by the next call,
      without a restart (re-read per call, cache invalidated on write).
- [ ] An unreadable configured token fails closed, naming the owner.
- [ ] `create_pr` does not push when the remote branch equals the local
      commit, and does push when it differs.
- [ ] Tests cover owner parsing (HTTPS, SSH, scp, PR URLs, case-insensitive),
      token selection, fallback, fail-closed, error attribution, cache TTL and
      invalidation, and the skipped push.

## Clarifications (resolved 2026-10-01, `/speckit.clarify`)

- **1Password token for server operations.** Resolution uses the VK service
  environment's `OP_SERVICE_ACCOUNT_TOKEN`, which is the existing
  `resolve_environment_secrets` fallback. Org tokens are machine-scoped, not
  tied to a workspace organization. Many server operations, such as the PR
  monitor, have no workspace organization to ask, and fetching org Env Vars
  from the remote on every PR call would add a network dependency. When that
  token is missing, the owner is *unavailable* and fails closed (FR-4), with
  the existing "1Password references require …" message.
- **SSH remotes.** When the owner has an org token, the server operation
  contacts the equivalent `https://github.com/<owner>/<repo>.git` URL for that
  one command. FR-2 requires the org token to be the only credential, and an
  SSH key would bypass it. Owners without a token keep SSH unchanged.
- **Clean-worktree check when the push is skipped.** Skipped. Uncommitted
  changes are never part of a PR. The check exists to protect a push, and with
  no push there is nothing to protect.
- **Cache lifetime.** 60 seconds, keyed by the stored row's id, last-update
  time and reference, and cleared on any Settings write. A Settings edit is
  therefore seen immediately, and only an in-place 1Password rotation behind
  an unchanged reference waits up to 60 s.

## Open Questions

None remaining.
