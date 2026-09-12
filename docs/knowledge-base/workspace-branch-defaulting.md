# Workspace branch defaulting

New workspaces must derive their base from repository intent, not from the
registered source checkout's current branch. A checkout under `/srv/src` may be
on a deployment, recovery, or feature branch without that branch becoming the
default for unrelated workspaces.

## Canonical policy

Frontend repository selectors should call
`packages/web-core/src/shared/lib/defaultBranch.ts` rather than duplicate branch
ordering. The policy is:

1. a valid explicit workflow initial branch, when that workflow supports one;
2. a valid user-configured `default_target_branch`;
3. `origin/main`;
4. `origin/master`;
5. the current checkout branch;
6. the first available branch, or `null` for an empty list.

A manual per-repository override remains above inference. The explicit initial
branch is handled by the calling hook before invoking `resolveDefaultBranch`;
the helper owns configured-default and fallback ordering.

## Remote refs are part of the contract

Branch names from the repository API retain their remote prefix. Workspace
inputs therefore persist `origin/main`, not a normalized `main`. Backend and
clustered-store consumers must resolve or materialize that exact remote-tracking
ref. Do not strip the prefix: a local `main` can point at a different commit.

## Regression boundary

Test selectors at the workspace-input boundary, not only at the pure helper.
The important regression fixture has a current local deployment branch plus
`origin/main` and asserts that the emitted input contains exactly
`target_branch: "origin/main"`. Also preserve configured-default, explicit
initial-branch, and manual-override precedence.

## Contributing tasks

- `vk/c59f-default-to-origi`
- `vk/b72a-internal-error-o`
- `vk/1476-protect-git-repo`
