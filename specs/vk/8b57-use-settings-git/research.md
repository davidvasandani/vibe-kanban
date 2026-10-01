# Research: server-side org-token credentials

## R1: How to inject a token into server git commands
- **Chosen:** per-`Command` env with `VK_GITHUB_PAT_<HEX>`, plus
  `GIT_CONFIG_PARAMETERS` holding a reset and the inline helper for
  `credential.https://github.com/<owner>`. This text is produced by the
  existing `utils::github_auth::routing_environment` and was verified with
  real git in `vk/0f52`.
- **Rejected:** an authenticated URL (`https://x-access-token:T@github.com/…`),
  because the token would leak into argv, `ps`, git error messages and
  trace logs.
- **Rejected:** `git -c credential.helper=…` with the token in the helper
  text, because the token would be in argv.
- **Rejected:** writing a repo or global config, because it persists and
  leaks across concurrent requests (constitution XLV).

## R2: How to inject a token into `gh`
- **Chosen:** `GH_TOKEN` in the child env, removing `GITHUB_TOKEN`,
  `GH_ENTERPRISE_TOKEN`, `GITHUB_ENTERPRISE_TOKEN` and
  `VK_GITHUB_ROUTED_OWNERS`. `GH_TOKEN` takes precedence over the
  hosts.yml login. `gh api` sends it as the Authorization header for REST and
  GraphQL.
- **Rejected:** replacing `gh` with a reqwest REST client. That is a large
  rewrite of the PR tooling recently hardened in `vk/53bc`, for no
  credential benefit.

## R3: Where resolution lives
- **Chosen:** the async resolver in `services` (it has DB, the key store and
  1Password). It produces a pure `GitHubCredentials` value, defined in
  `utils`, which the sync `git`/`git-host` crates consume. This keeps the
  dependency direction intact (`git`/`git-host` do not depend on `services`).
- **Rejected:** a process-global resolver hook called from inside `GitCli`.
  It would need a sync-to-async bridge, and its dependencies would be
  invisible at call sites.

## R4: Which owners to resolve for a repo
- **Chosen:** every remote of the repository. `resolve_pr_target` scans all
  remotes, and `push_to_remote` picks the default remote internally. Resolving
  all of them means whichever URL git contacts already has its credential.
  There are usually 1–2 owners, so the cost is small. Literal tokens need no
  I/O beyond a decrypt, and op reads are cached.

## R5: Cache
- 60 s TTL, keyed by `(row id, updated_at, normalized reference)`, cleared on
  Settings writes. `updated_at` uses `datetime('now','subsec')`, but the
  explicit invalidation covers same-millisecond saves anyway.

## R6: Push-skip authority
- **Chosen:** `git ls-remote <url> refs/heads/<branch>` compared with the
  local branch tip. A stale local `origin/<branch>` could equal HEAD after the
  remote branch was deleted. Skipping would then make PR creation fail with an
  unclear error, whereas pushing would have fixed it.

## New dependencies
None.
