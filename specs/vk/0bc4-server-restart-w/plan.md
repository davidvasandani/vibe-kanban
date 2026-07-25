# Implementation Plan: Never discard uncommitted worktree work on restart

**Spec**: `./spec.md`
**Status**: Draft

## Technical Context

Rust workspace (edition 2024), `crates/` layout, SQLite via SQLx, Tokio.
No frontend surface: this change is entirely backend lifecycle code, so
principle IV (shared-component boundaries) does not apply and no generated
types change.

Three crates are in scope, in dependency order:

- `crates/git` — `GitService` / `GitCli`. **Read-only for this change**; it
  already exposes everything needed (`get_worktree_change_counts`).
- `crates/worktree-manager` — owns the destructive leaf
  (`comprehensive_worktree_cleanup`). Has tests as of #151. Depends on `git`.
- `crates/workspace-manager` — owns the orphan sweep. Depends on `git` and
  `worktree-manager`. **Has no test infrastructure at all**; this change adds
  the first.
- `crates/local-deployment` — owns `kill_all_running_processes`. Has two test
  modules, both covering pure helpers/in-memory registries; nothing constructs
  a `LocalContainerService`.

Baseline confirmed green before starting: `cargo test --workspace` exits 0.

## Architecture & Approach

Two independent changes. They share the theme of principle XV but no code, and
are sequenced so each can be verified separately.

### Change 1 — FR-1, FR-2: preserve WIP regardless of stop outcome

**File**: `crates/local-deployment/src/container.rs`, `kill_all_running_processes`
(~line 2668).

Today the loop body is:

```rust
if let Err(error) = self.stop_execution(&process, Interrupted).await {
    tracing::error!("Failed to cleanly kill running execution process ...");
} else {
    tracing::info!("Successfully killed process: id={}", process.id);
    if let Err(error) = self.commit_interrupted_wip(&process).await { ... }
}
```

Restructure so the stop outcome is logged in a `match`/`if let` that produces no
control flow, and the `commit_interrupted_wip` call sits **after** it at a single
unconditional call site. The point is structural: with one call site outside any
branch, the defect cannot recur through later edits.

Ordering is preserved deliberately — stop is still attempted *before* preserve,
per the knowledge-base invariant that recovery kills the writer first and only
then snapshots. This is not reordered into snapshot-then-kill.

Per FR-2 the failure log is raised to `error!` and carries the workspace id
alongside the process id. `commit_interrupted_wip` already aggregates per-repo
failures into its error string, so repo names reach the log without extra work,
and its multi-repo best-effort semantics (attempt every dirty repo, refresh every
`after_head_commit`, then return an aggregate error) are left untouched.

**Explicitly not done** (spec C-1): the `Running` row left by `stop_execution`'s
early return is *not* changed to `Interrupted`. That row state is what lets the
next startup's `cleanup_orphan_executions` find and rescue the process. "Fixing"
it would delete the backstop.

### Change 2 — FR-3..FR-7: guard and instrument the orphan sweep

**File**: `crates/workspace-manager/src/workspace_manager.rs`.

`is_container_clean` cannot be reused: it takes a DB `Workspace` and enumerates
`WorkspaceRepo` rows, but an orphan candidate is by definition absent from the
DB. So the probe must work from the filesystem alone. Add a free function:

```rust
/// Whether an orphan-candidate directory holds work that must not be destroyed.
/// `Err` means it could not be determined — callers must retain.
fn workspace_dir_unsaved_work(workspace_dir: &Path) -> Result<Option<UnsavedWork>, WorkspaceError>
```

Resolution rules, chosen to bound a disk leak without ever trading away data:

| Subdirectory state | Decision |
| --- | --- |
| No `.git` marker in any subdir | No git-tracked work → deletable |
| `.git` present, probe succeeds, counts are 0 | Clean → deletable |
| `.git` present, probe succeeds, counts > 0 | **Retain**, report counts |
| `.git` present, probe fails | **Retain** (cannot determine) |

The middle rows use `GitService::get_worktree_change_counts`, which counts staged
*and* untracked changes — spec C-3, matching the expiry-sweep guard and #151's
preserve-aside check so all three retention decisions agree. `workspace-manager`
already depends on `git`, so no new dependency edge and nothing to record under
the constitution's new-dependency constraint.

The first row is what keeps FR-6 satisfiable: a directory with no git worktrees
at all cannot hold uncommitted *git* work, so genuinely dead directories are
still reclaimed. Without it, retain-on-error would leak every broken directory
forever.

Wire it into `cleanup_orphans_in_directory` (~line 557) between the
`container_ref_exists` check and the call to `cleanup_workspace_without_repos`,
and satisfy FR-5 by logging, before acting: the path, that it was selected
because no workspace record referenced it, and the action — including the dirty
counts when retaining.

Two smaller fixes in the same area:

- **FR-7**: `cleanup_workspace_without_repos` (~line 612) currently swallows a
  failing final `remove_dir_all` into a `debug!` and returns `Ok(())`, so the
  caller logs "Successfully removed orphaned workspace" when nothing was
  removed. Propagate the error.
- **FR-5** at the leaf: in `crates/worktree-manager/src/worktree_manager.rs`,
  `comprehensive_worktree_cleanup` (~line 385) logs its `git worktree remove
  --force` and its `remove_dir_all` at `debug!`, i.e. invisibly in production —
  the report's central complaint. Promote those to `info!`. This function is on
  the normal recreation path too, so the messages must stay terse.

## Data Model

Not applicable. No schema change, no migration, no new entity, and no change to
any type crossing the Rust/TypeScript boundary — so no `generate-types` run is
required. `data-model.md` is intentionally absent.

## Contracts

Not applicable. No HTTP route, MCP tool, or public API signature changes. The
one new function is crate-private. `contracts/` is intentionally absent.

## Research Notes

See `./research.md` for the severity re-assessment that inverted the obvious fix
(C-1), the startup-ordering race that reprioritised the two defects, and the
alternatives considered for the cleanliness probe.

## Constitution Check

- **I. Clarity over cleverness** — Both changes are small and local. The probe's
  decision table is stated explicitly rather than being implied by control flow.
- **II. Test the contract** — Change 2 gets real `#[cfg(test)]` coverage
  (AC-2/3/4) using `tempfile` and real `git`, mirroring the fixture style of
  `crates/git/tests/git_workflow.rs` and #151's tests. Change 1 is a
  control-flow correction in a function that no existing test can construct;
  it is validated by the end-to-end reproduction (AC-5). This asymmetry is
  recorded honestly rather than padded with a tautological unit test — see
  Risks.
- **III. Small, reversible steps** — Two independent edits, each revertible
  alone. No new plumbing; the probe reuses `get_worktree_change_counts`.
- **VI. Don't rebuild what shipped** — The largest contribution of this plan is
  what it *excludes*: #151's repair-first recreation, `.recovered-*` move-aside,
  and expiry guard are already correct and are left alone.
- **XII. Asynchronous handoffs have one authoritative owner** — Relevant, and
  respected: the orphan sweep and the startup WIP capture race today, and the
  guard removes the destructive half of that race rather than adding
  coordination locks.
- **XV. Destructive operations fail safe and are loud** — This change is the
  principle's first application: establish emptiness before deleting, retain
  when undeterminable, and log target/reason/action at `info!` before acting.
- **XIV. Repository verification is worktree-safe** — `pnpm install
  --frozen-lockfile` was run before verification, per the fresh-worktree rule.

No deviations.

## Risks & Dependencies

- **Retain-on-error could leak disk.** Mitigated by the "no `.git` marker
  anywhere → deletable" rule, which bounds retention to directories that
  genuinely look like worktrees. Residual risk: a workspace whose source repo was
  deleted keeps failing its probe and is retained indefinitely. Accepted — a
  bounded disk leak is strictly preferable to data loss, and `DISABLE_WORKTREE_CLEANUP`
  plus manual deletion remain available.
- **Change 1 has no unit test.** The function requires a DB-backed
  `LocalContainerService` that no existing test constructs; building that harness
  is disproportionate to a control-flow fix. Mitigated structurally (single
  unconditional call site) and behaviourally (AC-5 reproduction). Stated in the
  PR rather than papered over.
- **Promoting leaf logs to `info!` adds noise** on the normal recreation path.
  Accepted deliberately: silent destruction is the reported bug.
- **New test infrastructure in `workspace-manager`** means adding `tempfile` as a
  dev-dependency and tests that shell out to real `git`, which is slower and
  needs `git` on PATH. This matches what `crates/git` and `worktree-manager`
  already do, so it introduces no new class of requirement.
- **Depends on** `git` being present at test time, and on `get_worktree_change_counts`
  keeping its staged+untracked semantics.
