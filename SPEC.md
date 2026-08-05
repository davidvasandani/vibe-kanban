# Technical Spec: GitHub Fine-Grained PAT Routing by Repository Owner

Task id: `5e29-vk-github-fine-g`

## Summary

Allow a Vibe Kanban deployment to configure one GitHub fine-grained personal
access token (PAT) per GitHub organization or repository owner. Every Vibe
Kanban workspace process must receive a `gh` command that selects the token for
the repository targeted by that invocation, so a workspace containing repos
from multiple owners can use the GitHub CLI without manually switching
credentials.

The feature is limited to the Vibe Kanban source repository and its deployment
module, `homelab/modules/vibe-kanban-rebuild.nix`.

## Problem

`gh` normally chooses credentials by hostname. All GitHub.com organizations use
the same hostname, so setting one `GH_TOKEN` or one `github.com` entry in
`hosts.yml` cannot choose among fine-grained PATs belonging to different orgs.
A multi-repository workspace therefore needs command-time routing based on the
repository that `gh` will operate on.

## Goals

- Configure an owner-to-PAT mapping without putting PAT values in Git or the
  Nix store.
- Make routing work for coding agents, setup/cleanup scripts, dev servers,
  interactive workspace terminals, and cluster-dispatched executions.
- Select by explicit `gh -R/--repo` target when present; otherwise select from
  the current repository's GitHub remote.
- Support SSH and HTTPS GitHub remotes, case-insensitively.
- Preserve ordinary `gh` behavior when no configured owner can be determined.
- Avoid leaking PATs through logs, process arguments, generated prompts, or
  files inside a workspace.
- Provide deterministic errors when an owner is recognized but its credential
  is missing or unreadable.

## Non-goals

- Managing GitHub tokens for any service other than Vibe Kanban.
- Creating, rotating, or validating PAT scopes through the GitHub API.
- Changing Git credentials used by `git fetch`, `clone`, or `push`.
- Supporting GitHub Enterprise hosts in the first version.
- Selecting different PATs for two repositories with the same owner.
- Persisting secrets in Vibe Kanban's local or remote databases.

## Proposed design

### Deployment configuration

Extend `services.vibe-kanban-rebuild` with a `githubOrgTokens` attribute set.
Keys are GitHub owner names. Values identify runtime-only secret sources using
the module's existing 1Password/systemd credential conventions. Evaluation
must reject invalid/ambiguous owner keys and secret paths that point into the
Nix store.

At service startup, a root- or service-owned preparation step resolves the
configured secrets into a single credential directory outside the Nix store.
Files are mode `0400`/`0440`, readable only by the Vibe Kanban runtime identity
needed on that node. Token contents are never interpolated into Nix-generated
scripts or unit environment values. Coordinator and worker nodes receive the
same owner mapping because either can launch workspace processes.

The runtime gets only:

- a non-secret owner-to-credential-file manifest path; and
- a wrapper directory prepended to execution `PATH`.

### `gh` routing wrapper

Ship a small, testable wrapper with the Vibe Kanban deployment/runtime. The
wrapper invokes the real GitHub CLI after resolving an owner:

1. Parse global `gh` arguments for `-R OWNER/REPO`, `--repo OWNER/REPO`, or
   `--repo=OWNER/REPO`. This explicit target has highest precedence.
2. Otherwise ask Git for the effective repository remote in the current
   directory, honoring `remote.pushDefault`, branch push remote, branch remote,
   and then `origin` as practical fallbacks.
3. Parse only GitHub.com SSH/HTTPS remote forms and extract the first path
   segment as the owner.
4. Match the owner case-insensitively against the configured manifest.
5. Read the selected token from its runtime credential file and export it as
   `GH_TOKEN` only in the child process, then `exec` the real `gh` binary.
6. If no configured owner matches, invoke real `gh` unchanged so existing
   authentication continues to work.

An already-set `GH_TOKEN` must not silently defeat repository routing. For a
configured owner the routed PAT wins; for an unconfigured/unknown owner the
caller's environment remains untouched. The wrapper must prevent recursion by
using an absolute path to the packaged real `gh` executable.

Commands that target multiple repositories in one invocation are outside the
normal `gh` contract; the single explicit/current repo determines the token.

### Workspace process integration

The wrapper path and manifest pointer are execution-owned configuration. Add
them at the common workspace environment boundary and to workspace PTYs, with
the same coverage and precedence rules as other Vibe Kanban-owned variables.
Cluster dispatch must transmit the non-secret routing configuration or install
equivalent node-local paths; it must never serialize PAT contents into the
coordinator-to-worker action payload.

Non-workspace administrative/login PTYs do not inherit this configuration.

### Security behavior

- Secret values remain runtime credentials and are exposed only as `GH_TOKEN`
  to the invoked `gh` child.
- No token value may be formatted with `Debug`, emitted in tracing, returned by
  an API, or copied into the workspace.
- Manifest and errors may name configured owners and credential paths, but not
  token contents.
- Symlink and permissions checks protect credential reads; empty tokens are
  treated as configuration errors.
- Owner parsing is strict to avoid selecting credentials for lookalike hosts or
  malformed repository strings.

## Functional requirements

1. A workspace with `org-a/repo-a` and `org-b/repo-b` uses org A's PAT when
   `gh` runs from repo A and org B's PAT when it runs from repo B.
2. `gh -R org-b/repo-b ...` from repo A uses org B's PAT.
3. SSH (`git@github.com:Owner/repo.git`) and HTTPS
   (`https://github.com/Owner/repo.git`) remotes resolve identically.
4. Owner matching ignores case, while repository names are not otherwise
   rewritten.
5. Unconfigured owners and non-GitHub remotes retain existing `gh` auth.
6. Missing/empty configured credentials fail before contacting GitHub and name
   the affected owner without printing the token.
7. Routing is available in every workspace-owned child-process path on both
   coordinator and worker nodes.

## Verification

- Unit tests for explicit repo parsing, argument precedence, remote URL forms,
  owner normalization, malformed inputs, and fallback behavior.
- Integration tests with a fake real `gh` executable that records only which
  synthetic token identifier it received; no real credential is used.
- Process-boundary tests for managed executions and workspace PTYs.
- Nix evaluation tests/assertions for valid configuration, invalid owner names,
  runtime-only credential paths, and disabled/default behavior.
- Repository formatting, targeted Rust/frontend tests if source changes are
  needed, `pnpm run check`, and the relevant Nix checks.

## Acceptance criteria

- Per-owner PAT configuration is documented and deployed through
  `modules/vibe-kanban-rebuild.nix`.
- Multi-owner workspaces route `gh` calls correctly without user intervention.
- PATs do not enter the Nix store, database, workspace filesystem, logs, or
  cluster dispatch payloads.
- Existing installations with no mapping behave exactly as before.
- All relevant tests and independent review pass.
# VAS-356 Addendum: Cluster-safe MCP runtime connectivity

MCP definitions can persist successfully while worker-hosted Codex cannot reach
their backends. VK workers must expose `worker.coordinatorUrl` as
`VIBE_BACKEND_URL`, and think1 must admit the configured VK workers to
Firecrawl TCP 3410 without widening logmein ports 8189/8190. Acceptance requires
evaluated worker environments, exact nftables scope, focused checks, and
post-deploy live MCP initialization from a worker.
