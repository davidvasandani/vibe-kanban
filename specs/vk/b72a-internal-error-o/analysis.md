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

---

# Post-implementation review round

Codex CLI is not installed on this worker and no `codex-review` skill is
registered, so the independent pass was run as five parallel reviewers (a
line-by-line diff scan, a removed-behaviour audit, a cross-file tracer, a
reuse/simplification/efficiency/conventions pass, and an adversarial reviewer
briefed to refute the change). Substituting them is recorded here rather than
claimed as a Codex run.

Four of the five converged on the same defects. All were in the change, not in
the diagnosis.

### R1 — the fix silently stopped working after the first workspace *(fixed)*

`git fetch` is **atomic across its refspecs**. Batching
`+refs/heads/*:refs/heads/*` with `+refs/remotes/*:refs/remotes/*` meant that
when the heads refspec was refused, the whole command aborted and wrote
nothing — including the remote-tracking mirror this change exists to add.

And the heads refspec is refused in the *steady state*, not a corner: once a
workspace exists the store has a worktree checked out on `vk/…`, and
`mirror_branch_back` has pushed that branch into the checkout, so git answers
`refusing to fetch into branch 'refs/heads/vk/…' checked out at …` (exit 128).
Reproduced on git 2.54.0.

So the first workspace of a repository would have worked and every later one
would have fallen back to a forge fetch — reintroducing the reported failure
with a nicer message. The `fetch_with_refspecs` helper added in T002 was
reverted; the two mirrors are now separate invocations with independent
error handling, and `fetch_with_refspec`'s doc comment states why.
Pinned by `the_remote_tracking_mirror_survives_a_checked_out_branch`.

### R2 — the store froze at the first commit it learned *(fixed)*

Widening `branch_commit_present` also widened `store_resolves`, which is
`ensure`'s early return. Once `refs/remotes/origin/main` existed, every later
`ensure` short-circuited before the mirror — so workspaces 2..N branched from
whatever commit the first one captured, while the picker showed the current
one. Silent, and `origin/main` is the default, so it would have been the common
case.

`store_resolves` now requires a *local head*: a remote-tracking ref is a copy of
something that moves and is never evidence the store is current. Pinned by
`a_moved_target_branch_is_picked_up_by_the_next_provisioning`.

### R3 — `adopt` could have emptied a live worktree's index *(fixed)*

`adopt`'s pre-mutation guard used the widened predicate, but `write_linkage`
writes `ref: refs/heads/{branch}`. A workspace branch matching only a
remote-tracking ref would have been adopted onto an unborn HEAD, and the
`git reset -q` that follows would have cleared the index instead of rebuilding
it — every tracked file in someone's work-in-progress reading as deleted, while
the function reported `Adopted`. Exactly the "half-adopted worktree ... looks
repaired" outcome its own comment says must be impossible.

The guard now requires a local head, so it agrees with the mutation. Pinned by
`adopt_refuses_a_branch_that_is_only_a_remote_tracking_ref`.

### R4 — a guaranteed-failing push per provisioning *(fixed)*

`mirror_branch_back` was gated by the widened predicate but pushes
`refs/heads/{branch}:refs/heads/{branch}`. For `origin/main` it spawned a push
whose source ref does not exist, once per repository per `ensure`, swallowed at
`debug!`. Now gated on a local head — and there is nothing to mirror back for a
ref that came *from* the checkout.

### R5 — the fallback loop could poison the shared store *(fixed)*

The loop asked *every* remote for the target branch. For a target prefixed with
one remote's name, the others received the local-to-local refspec — and a remote
that happened to hold a branch literally named `upstream/main` would have landed
it as a **local** head in the shared store, where local-first resolution then
prefers it forever, at the wrong commit, for every workspace on every node. The
loop now only asks the remote whose name prefixes the target branch.

### R6 — a failed fallback fetch was discarded *(fixed)*

The refusal message asserts the branch is not present. If the only attempt to
obtain it never ran — expired credentials, unreachable host,
`GIT_TERMINAL_PROMPT=0` declining — that assertion misdirects the investigation
this message exists to shorten. Failures are now logged and folded into the
error text.

### Considered and not changed

- **`+refs/remotes/*` is broader than the target branch needs.** Deliberate: it
  is what makes "the set you can pick equals the set the store can serve" true
  by construction, which is the property C2 chose. Narrowing it would reintroduce
  a per-branch fetch.
- **Delegating to git's bare-name revision precedence** instead of naming the two
  namespaces. Rejected and now tested: that precedence also accepts
  `refs/tags/<name>`, so a tag named `main` would satisfy a target branch, which
  `GitService::find_branch` never does (`a_tag_is_not_a_branch`).
- **A local target branch still short-circuits and can go stale.** Pre-existing
  behaviour, unchanged by this work, and fixing it means taking the lease on
  every `ensure`. Recorded as out of scope.
- **`fallback_refspec` duplicates the refspec shape in `GitService::fetch_branch_from_remote`.**
  Real duplication, but that function is private, takes a git2 `Reference`, and
  sits on the PR/rebase path with no coverage; refactoring it inside a bug fix is
  the scope creep the plan set out to avoid.
- **The error message discloses the store's absolute path.** It already did
  before this change, and for a self-hosted operator that is the point.
