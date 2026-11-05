# Feature Specification: Reliable workspace creation for large/foreign-hook repos

**Feature dir**: `specs/vk/0fab-vas-981-sg-monor/`
**Status**: Draft

## Summary
`start_workspace` against the sg-monorepo repository stopped producing a
working worktree/session: it returned only a workspace id, with no branch or
session, and the resulting record could not be inspected, retried, or cleaned
up through the normal MCP tools. Host investigation found two independent,
confirmed causes — a branch that exists only on the remote is never fetched
before workspace creation fails, and a workspace's own `lefthook install`
postinstall can write hook shims into the shared clone's common git
directory that then break every subsequent `git worktree add` for that repo.
In both cases the real failure reason was computed and logged server-side but
never recorded anywhere a caller (MCP tool, `create_session`) could see it,
so every follow-up call produced a generic, unexplained error instead.

## User Stories
- As an agent calling `start_workspace` on a branch that exists on the
  remote but hasn't been fetched locally yet, I want workspace creation to
  fetch it automatically, so I don't have to separately know about or work
  around the local clone's fetch state.
- As an agent or operator, when workspace creation fails for any reason, I
  want the real reason recorded on the workspace so that calling
  `create_session` (or inspecting the workspace) tells me what actually
  went wrong, instead of a generic "not found" error that looks like the
  workspace itself vanished.
- As the Vibe Kanban service, I want my own git plumbing (worktree add,
  checkout, merge, rebase, fetch, push, commit) to be unaffected by any hook
  a repository's own tooling installs into a shared checkout, so one
  workspace's `postinstall` can never break workspace creation for every
  other workspace on that repository.

## Functional Requirements
- FR-1: When a workspace's requested branch does not exist in the repo's
  local checkout, the system must check whether it exists on the
  repository's configured remote, and if so, fetch it before re-checking.
  Only if the branch is absent from both local and remote does creation fail
  with a branch-not-found error.
- FR-2: Every git command the system runs against a repository's checkout
  (as part of worktree administration, branch management, or workspace
  creation) must run with that repository's hooks disabled, regardless of
  what any workspace has written into the shared checkout's hook directory.
- FR-3: When workspace creation fails in the background after the initial
  response has already been returned, the system must record the actual
  failure's message on the workspace record — not a generic placeholder —
  so it is visible to anything that later inspects that workspace.
- FR-4: A request to create a session on a workspace must not succeed, and
  must not emit a misleading "workspace not found" error, when the
  workspace's creation is still pending or has failed. It must identify
  which of those two states applies and, for a failure, include the
  recorded reason.

## Out of Scope
- Changing the asynchronous, fire-and-forget nature of workspace creation
  itself (a slow-but-successful creation, e.g. a large repo's NFS checkout,
  is expected and not a defect).
- Any fix to `restart_workspace`/`delete_workspace` routing: investigation
  found no bug in this checkout's `delete_workspace` (plain DB lookup, no
  worktree precondition); `restart_workspace` could not be located in this
  checkout at all and is flagged as an open question rather than guessed at.
- Any change to the shared sg-monorepo clone's filesystem state — the
  immediate hook breakage was already remediated by hand on the host.

## Acceptance Criteria
- [ ] Starting a workspace on a branch that exists only on the remote
      (e.g. an open PR's branch) succeeds without the caller needing to
      pre-fetch it, or fails with a message naming the branch and repo.
- [ ] A workspace whose creation fails for any reason has that reason
      recorded and inspectable; `create_session` against it returns that
      reason instead of "Workspace not found".
- [ ] A workspace whose creation is still in progress returns a distinct
      "still being created" response from `create_session`, not "Workspace
      not found".
- [ ] A hook written into a shared repository checkout by a workspace's own
      tooling cannot break `git worktree add` (or any other vk-issued git
      command) for any other workspace on that repository.

## Open Questions
- Deferred, non-blocking: where does the live `restart_workspace` MCP tool's
  handler actually live? It responded during live investigation but no
  route/handler by that name exists in this checkout. Already called out as
  explicitly out of scope for this fix (see "Out of Scope"); no FR or
  acceptance criterion depends on resolving it, so it does not block
  planning or implementation. Left for the user/operator to locate.
