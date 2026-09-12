# Feature Specification: GitHub PAT Routing by Repository Owner

**Feature dir**: `specs/vk/5e29-vk-github-fine-g/`
**Status**: Draft

## Summary

Let an operator associate a GitHub fine-grained personal access token with each
GitHub organization or repository owner used by Vibe Kanban workspaces. When a
user or coding agent invokes the GitHub CLI, Vibe Kanban selects the credential
for the repository that command targets. This makes multi-repository workspaces
usable when their repositories belong to different owners and no one token is
authorized for all of them.

## User Stories

- As an operator, I want to configure a distinct GitHub PAT for each GitHub
  organization so that I can grant only the repository access each org needs.
- As a coding-agent user, I want `gh` to use the correct identity automatically
  when I work in any repository in a multi-repository workspace.
- As a user, I want an explicit `gh --repo OWNER/REPO` target to choose the
  matching credential even when my current directory belongs to another repo.
- As a security-conscious operator, I want PATs to remain outside source,
  workspace data, logs, and cross-node messages.

## Functional Requirements

- FR-1: The deployment must accept zero or more associations between a GitHub
  owner and a runtime-provisioned fine-grained PAT.
- FR-2: Owner associations must be unique without regard to letter case.
- FR-2a: Each owner association identifies a 1Password item/field reference;
  the deployment resolves those references at service start through its
  existing runtime-only 1Password bootstrap credential.
- FR-3: Every `gh` invocation from a workspace-owned process must determine its
  target owner from an explicit repository argument when one is supplied.
- FR-4: When no explicit repository is supplied, the invocation must determine
  its target owner from the effective GitHub repository for the current working
  directory. Remote selection follows, in order: `remote.pushDefault`, the
  current branch's `branch.<name>.pushRemote`, its `branch.<name>.remote`,
  `origin`, and then the sole configured remote.
- FR-5: Explicit repository context must take precedence over inferred current
  repository context.
- FR-6: GitHub.com HTTPS and SSH repository forms must resolve to the same owner,
  and owner matching must be case-insensitive.
- FR-7: For a configured owner, the invocation must expose only that owner's PAT
  to the GitHub CLI process. The configured per-owner PAT overrides an ambient
  caller `GH_TOKEN` for that invocation.
- FR-8: For an owner that is not configured, a non-GitHub repository, or a
  directory without repository context, the invocation must preserve the
  existing GitHub CLI authentication behavior.
- FR-9: If an owner is configured but its PAT cannot be read or is empty, the
  invocation must stop with an actionable error naming the owner without
  revealing any secret value.
- FR-10: Routing must apply to coding agents, follow-up and review executions,
  repository lifecycle scripts, development servers, and interactive terminals
  belonging to a workspace.
- FR-11: Routing must behave the same whether the workspace process runs on the
  coordinator or an assigned cluster worker.
- FR-12: Non-workspace authentication/login terminals must not inherit the PAT
  routing configuration.
- FR-13: PAT contents must not be stored in the Nix store, either Vibe Kanban
  database, shared workspace files, prompts, logs, API responses, or serialized
  coordinator-to-worker messages.
- FR-14: An installation with no configured owner associations must retain its
  current behavior.
- FR-15: Operator documentation must explain configuration, target precedence,
  supported repository forms, fallback behavior, rotation, and safe diagnosis.

## Out of Scope

- Creating, rotating, or inspecting PAT permissions through GitHub.
- Authentication for `git clone`, `fetch`, `pull`, or `push`.
- GitHub Enterprise Server or owners on hosts other than GitHub.com.
- More than one PAT for repositories under the same GitHub owner.
- Changes to services other than Vibe Kanban and its governing deployment
  module.
- A browser UI for viewing or editing PAT values.

## Acceptance Criteria

- [ ] In a workspace containing repositories owned by `org-a` and `org-b`, a
      `gh` invocation from each repository receives that owner's configured PAT.
- [ ] From the `org-a` repository, `gh --repo org-b/another-repo ...` receives
      the `org-b` PAT.
- [ ] Equivalent HTTPS and SSH remotes, including owner case differences,
      select the same configured owner.
- [ ] A command for an unconfigured owner receives no routed PAT and uses the
      pre-existing authentication path.
- [ ] A configured owner with a missing or empty credential fails locally with
      a secret-safe message that names the owner.
- [ ] Synthetic process-level tests demonstrate routing in managed workspace
      execution and workspace PTY paths.
- [ ] Cluster tests or contract inspection demonstrate that a worker uses its
      node-local credential and no PAT is present in the dispatched payload.
- [ ] Nix evaluation rejects case-insensitive duplicate owners and secret paths
      that would enter the Nix store.
- [ ] Searching tracked changes and captured test output finds no real or
      token-shaped PAT value.
- [ ] With the association map empty, the relevant behavior and tests are
      unchanged.

## Open Questions

None.
