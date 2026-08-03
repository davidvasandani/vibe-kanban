# Analysis — `b72a-internal-error-o`

Cross-check of [`spec.md`](spec.md), [`plan.md`](plan.md) and
[`tasks.md`](tasks.md) against
[`../../../.specify/memory/constitution.md`](../../../.specify/memory/constitution.md)
(v0.19.0), before implementing.

## Requirement → task coverage

| Req | Implemented by | Proven by | Status |
| --- | --- | --- | --- |
| FR-1 remote-prefixed branch provisions | T005, T006 | T012 | covered |
| FR-2 local-then-remote, local wins | T005 | T013 | covered |
| FR-3 presence proven, not inferred | T005 | existing `commit_presence_is_proven_not_assumed` | covered, see W1 |
| FR-4 store holds the pickable refs | T006 | T012 | covered |
| FR-5 mirroring never removes refs | T006 | — | **gap, see E1** |
| FR-6 no unsatisfiable network fetch | T007 | T014 | covered, see I1 |
| FR-7 attributed failure at the API | T008, T009, T010 | T015 (crate-level only) | **gap, see E2** |
| FR-8 unchanged off the cluster path | — (by construction) | — | see I2 |
| FR-9 local branch names still work | — (unchanged) | existing suite | covered |

## Findings

### E1 — error — `tasks.md`: FR-5 has no test

FR-5 ("populating the store never removes refs") is the one requirement whose
violation would be silent and would damage *other* workspaces rather than the
one being created. Constitution XX makes it load-bearing: writes into a
consolidated shared namespace are additive by default, and `refs/remotes/*` in
the store is shared by every workspace of that repository on every node.

T006 states "no `--prune`" as an instruction, and nothing checks it. An
instruction in a task list is not a control — the same objection the constitution
raises against a one-off migration with no recurring check.

**Remedy:** extend T012 to seed a ref in the store that the registered checkout
does not have (a `vk/…` head belonging to a notional other workspace) and assert
it survives `ensure`. One assertion, no new fixture.

### E2 — error — `tasks.md`: FR-7's user-visible half is untested

The whole point of the error work is what reaches the operator. T015 asserts
`ensure` returns `WorkspaceError::SharedStore`, but the value only becomes
non-generic after two more hops — `map_workspace_manager_error` (T009) and the
`From<ContainerError>`/render match (T010). Those hops are exactly where the
message is lost today, and both are one-line match arms that a future edit could
re-route into the `other =>` catch-all without failing a single test.

**Remedy:** add a test in `crates/server/src/error.rs` asserting that
`ApiError::from(ContainerError::SharedStore(msg))` renders 500 with `msg`
verbatim and *not* the generic string. Cheap, and it pins the arm ordering the
seam document calls out.

### W1 — warning — `tasks.md`: AC 4 is claimed by an existing test not named anywhere

AC 4 ("a well-formed but absent object is not treated as present") is satisfied
by `commit_presence_is_proven_not_assumed`, which T005 must keep green. `plan.md`
Step 6 says so; `tasks.md` does not, so a reader working only from the task list
could rewrite that test while "refactoring" `branch_commit_present`.

**Remedy:** name it in T005 as a task the change must not modify.

### W2 — warning — constitution XXI: the resolution rule is duplicated, not reused

XXI says a value with an existing resolution rule is resolved *by that rule*, and
re-deriving it "is a defect even when it passes its own tests". `resolved_branch_ref`
does re-derive `GitService::find_branch`'s local-then-remote order, in a second
implementation (`GitCli` subprocess) against a second backend (git2).

The duplication is justified and cannot be removed: `find_branch` needs a
`git2::Repository` and answers "does this ref exist", whereas constitution XX
requires the store to prove the *commit object* is present — a strictly stronger
question that `find_branch` does not answer. Reusing `find_branch` here would
trade an XX violation for an XXI one.

**Remedy** (rather than an exception): make the duplication *pinned* instead of
merely documented — a test asserting the two resolvers agree across the truth
table. That is what turns "we matched the rule" into a control, and it is the
cheapest available form of the reuse XXI actually wants.

### I1 — info — `spec.md` FR-6 is satisfied for the reported case, not universally

`fallback_refspec` corrects the refspec when the target branch is prefixed with
*this* remote's name. A target prefixed with a *different* remote's name (target
`origin/main` while iterating a remote called `upstream`) still produces one
doomed fetch, unchanged from today.

Deliberately left alone: the store's remotes are copied from the registered
checkout, where `origin` is present and first, so the reported case is fully
covered; the loop is bounded; and narrowing it further would change behaviour for
plain local branch names, which FR-9 protects. Constitution III (smallest change
that delivers value) favours leaving it.

### I2 — info — FR-8 is unreachable-by-construction, and that is adequate

Nothing changes for local placements or clustering-disabled installs because
`shared_repository_store_for` returns `None` for `WorkspacePlacementState::Local`
and when `cluster_config.enabled` is false, so no code touched by this change is
ever constructed. That match is already exhaustive-on-purpose (a new placement
state is a compile error there), which is a stronger guarantee than a test.

### I3 — info — `plan.md` risk "a store created before this change lacks the remote refs" is correctly handled

`ensure` runs on every provisioning and is idempotent, so the first cluster
workspace created for a repository after the fix backfills `refs/remotes/*`. The
two live stores on the share will self-heal without a migration. Confirmed
against their current contents, which hold `refs/heads/*` only.

### I4 — info — no constitution violations found in the mechanism itself

- XVIII: mirroring stays coordinator-only, inside the per-repository fenced
  lease. Unchanged.
- XX: presence proven with `cat-file -e`; no textual path checks; additive
  mirroring (subject to E1).
- XV: nothing on this path deletes or overwrites a working tree.
- XI: the backend message reaches the UI verbatim via `displayError`.
- Constraints: no new dependency; no generated file edited.

## Disposition

E1, E2, W1 and W2 are folded into `tasks.md` as T012a, T013a, T015a and an
amendment to T005 before implementation begins. I1–I4 are recorded, not acted
on.
