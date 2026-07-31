# Prior Knowledge: Clustered Vibe Kanban

The project knowledge base is populated. The most relevant pages are:

- `docs/knowledge-base/interrupted-worktree-recovery.md`
- `docs/knowledge-base/workspace-directory-reclamation.md`
- `docs/knowledge-base/workspace-environment-inheritance.md`
- `docs/knowledge-base/cli-tool-oauth-login.md`
- `docs/knowledge-base/issue-status-side-effects.md`

## Distilled Guidance

1. An execution's Git snapshot and `execution_process_repo_states` must describe
   the same durable state. Reconciliation must stop or establish loss of the
   writer before WIP capture, attempt preservation even when teardown fails, and
   never infer completion from absence.
2. Current restart recovery deliberately leaves some shutdown-side rows
   `Running` so the next boot can capture work. Remote reconciliation must
   replace that implicit local-process assumption with explicit worker evidence
   without removing the preservation backstop.
3. Multi-repository WIP capture is best-effort and must refresh metadata for
   every repository after partial success. A failed repository excludes the
   execution from transparent auto-resume.
4. Vibe Kanban has two destructive workspace sweeps: DB-known expiry and
   filesystem-only orphan cleanup. Both must retain data when cleanliness or
   ownership is indeterminate. A disconnected assigned worker makes ownership
   indeterminate.
5. Orphan classification currently uses exact `container_ref` string matching,
   and the startup orphan sweep can race execution recovery. Shared canonical
   paths and persisted affinity must be considered before either sweep runs.
6. Repository-wide `git worktree prune` is a known concurrency hazard. Cluster
   work requires coordinator-only worktree administration and repository-scoped
   serialization; workers may use ordinary Git inside a worktree but must not
   add/remove/prune worktrees or delete branches.
7. Destructive filesystem/worktree actions must log target and reason at normal
   operational visibility before acting, and errors must not be turned into a
   successful cleanup report.
8. Workspace behavior spans multiple process boundaries. Setup, agents, dev
   servers, cleanup/review scripts, PTYs, and managed login/helpers do not all
   pass through one launcher today. Sticky affinity and environment delivery
   must cover every boundary, with terminal-owned values applied last.
9. Secrets should be resolved once per workspace/execution and injected only
   into the child process. Do not write secret `.env` files, mutate the
   coordinator/worker service environment, duplicate resolution logic, or
   debug-format resolved secret maps.
10. Existing host-scoped CLI/relay work already established that a path alone
    is insufficient to identify an execution target. Host identity must be
    passed and persisted explicitly.
11. Reconciliation should be level-triggered, idempotent, failure-isolated, and
    driven by authoritative snapshots rather than assuming a missed edge event
    will be replayed forever.

## Implications for This Task

- Make the worker plus workspace ID, not `container_ref` alone, the execution
  authority boundary.
- Run worker reconciliation before destructive cleanup and preserve any
  workspace whose worker/process state cannot be proven inactive.
- Retain existing local recovery semantics when clustering is disabled; remote
  execution needs explicit leases, job reports, and indeterminate states.
- Centralize child-process dispatch and workspace environment construction so
  later terminal/preview/helper support cannot silently fall back to the
  coordinator.
- Fix or fence repository-wide worktree administration before allowing two
  workers to execute against shared repository metadata.
