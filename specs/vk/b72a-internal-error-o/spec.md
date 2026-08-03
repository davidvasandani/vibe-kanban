# Feature Specification: Remote-tracking target branches for cluster-placed workspaces

**Feature dir**: `specs/vk/b72a-internal-error-o/`
**Status**: Draft
**Task id**: `b72a-internal-error-o`
**Branch**: `vk/b72a-internal-error-o` (already exists)

## Summary

Starting a new issue on a clustered Vibe Kanban coordinator returns
"An internal error occurred. Please try again." whenever the selected
repository's target branch is a remote-tracking name — which is the value the
create screen picks by default (`origin/main`). Provisioning a cluster workspace
now goes through a shared bare Git store, and that store decides whether it can
serve a branch by asking for `refs/heads/<branch>` only, so the default can never
resolve. This feature makes the shared store resolve a target branch the same way
the rest of the product does (local branch first, then remote-tracking branch),
gives the store the refs that resolution needs, and stops the failure from
reaching the user as an unattributable generic error.

Users see two symptoms today, and both come from this one cause: an error on
create, and — when the same request first spends a guaranteed-to-fail network
fetch trying to recover the missing branch — a long unexplained wait.

## User Stories

- As someone starting an issue on a clustered deployment, I want the repository
  and branch I picked to just work, so that I do not have to discover by trial
  that only *some* branch choices can be run on a worker.
- As someone starting an issue, I want the create request to fail fast rather
  than stall, so that a failure costs me seconds rather than minutes.
- As the operator of this deployment, I want a failed create to tell me what
  failed and for which repository, so that diagnosing it is a single read rather
  than an investigation across three hosts.
- As a maintainer, I want the shared store to agree with the rest of the product
  about what a branch name means, so that a future caller does not have to
  rediscover this class of bug.

## Functional Requirements

- **FR-1** A workspace whose repository's target branch is a remote-tracking
  name (`origin/main`, `origin/master`, `<remote>/<branch>` generally) is
  provisioned successfully on a worker, given that the name resolves in the
  registered checkout the branch picker read it from.
- **FR-2** Target-branch resolution in the shared store matches the product's
  existing rule — the local branch of that name if one exists, otherwise the
  remote-tracking branch of that name. A name that is both resolves to the local
  one.
- **FR-3** Presence is proven, not inferred: a branch counts as available only
  when its commit object is demonstrably present in the store.
- **FR-4** The shared store holds every ref a target branch could name, sourced
  from the same registered checkout the branch picker offered its choices from,
  so that the set of branches a user can pick and the set the store can serve are
  the same set. Populating it is best-effort — the presence check of FR-3 is the
  gate, not the copy's exit status.
- **FR-5** Populating the store never removes refs. It may add and fast-forward
  or force-update the refs it mirrors, and must not delete refs another
  workspace of the same repository may depend on.
- **FR-6** The system does not perform a network operation that cannot succeed.
  The recovery fetch that reaches the real forge is retained, for the case where
  the branch exists upstream but the registered checkout has not fetched it, but
  it is attempted only in a form the upstream could satisfy, and its success is
  judged by whether the branch is afterwards present — not by the command's exit
  status.
- **FR-7** When provisioning a cluster workspace fails because the shared store
  cannot serve the requested branch, the response names the repository, the
  branch, and what could not be done. It stays a server error (HTTP 500) so it
  remains logged server-side, but it is no longer indistinguishable from any
  other internal failure.
- **FR-8** Behaviour is unchanged for workspaces that do not use the shared
  store: coordinator-local placements and deployments with clustering disabled.
- **FR-9** Behaviour is unchanged for target branches that are plain local
  names; the previously working path stays working.

## Out of Scope

- Changing how the create screen picks a default branch. The picker matches
  branches by exact name against a list that legitimately contains
  remote-prefixed names; normalising there would break that matching and is
  explicitly recorded in the knowledge base as the wrong side of the seam.
- The dormant second branch selector (`useRepoBranchSelection` /
  `RepoBranchSelector`), which has no importers and divergent defaults.
- A request timeout or client-side abort for workspace creation. The stall this
  feature removes is one cause; the absence of any bound is a separate defect.
- Worker endpoint resolution caching only successful probes, so an unreachable
  configured endpoint costs a full client timeout on every miss.
- Rolling back the workspace row, its repository attachments, and its session
  when provisioning fails, so a failed create leaves nothing behind.
- Broadening error reporting for internal failures generally. Only the
  shared-store provisioning failure changes what a user sees.
- Freshening the coordinator's view of the forge. The store inherits exactly the
  registered checkout's freshness, which is the freshness the branch picker
  already showed the user; no new network fetch is added to the create path
  (see [`clarifications.md`](clarifications.md) C2).
- **A *local* target branch still short-circuits `ensure` and can go stale.**
  Pre-existing behaviour, unchanged here: once the store holds
  `refs/heads/main`, later provisionings return early and never refresh it. This
  work fixes the remote-tracking case (the default, and the reported one) and
  deliberately leaves the local case alone — closing it means taking the
  administration lease on every `ensure`, which is a different change with a
  different risk profile.
- **The heads mirror is refused once any workspace exists.** `git fetch` will
  not write `refs/heads/vk/…` while a worktree has it checked out, so
  `+refs/heads/*:refs/heads/*` fails in the steady state. That is pre-existing
  (#174 shipped it) and is why the two mirrors must be separate invocations —
  see [`analysis.md`](analysis.md) R1. Making the heads mirror itself work is
  out of scope, and force-updating a branch a live workspace is sitting on would
  need its own design.

## Acceptance Criteria

- [ ] With a repository whose checkout has a remote-tracking `origin/main` and
      no local branch of that name, provisioning a cluster workspace with target
      branch `origin/main` succeeds, and the resulting store resolves
      `origin/main`. (FR-1, FR-4)
- [ ] From that store, creating the workspace branch based on `origin/main` and
      registering a worktree for it both succeed. (FR-1, FR-4)
- [ ] Given both a local `shared` branch and a remote-tracking `origin/shared`
      at different commits, resolution returns the local one. (FR-2)
- [ ] A well-formed but absent object is not treated as present. (FR-3)
- [ ] A recovery fetch for `origin/main` targets the upstream branch `main` and
      lands it in the remote-tracking namespace; for a plain `main` it keeps the
      local-to-local form. (FR-6)
- [ ] Provisioning with a branch that exists nowhere still fails, and the
      failure names the repository and the branch. (FR-7)
- [ ] The new tests fail against unmodified `main` and pass after the change.
- [ ] `cargo test --workspace` and `cargo clippy --workspace --all-targets
      -- -D warnings` pass.
- [ ] Existing shared-store tests pass unchanged, pinning FR-9.

## Open Questions

All resolved — see [`clarifications.md`](clarifications.md):

- C1 mirroring is best-effort, the presence check is the gate.
- C2 the store inherits the registered checkout's freshness; no freshening
  fetch is added.
- C3 the forge recovery fetch is kept and corrected, not removed.
- C4 the failure is a 500 carrying an attributed message.
