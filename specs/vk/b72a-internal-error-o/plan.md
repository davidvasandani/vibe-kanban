# Technical Plan — `b72a-internal-error-o`

Spec: [`spec.md`](spec.md) · Clarifications: [`clarifications.md`](clarifications.md)
Evidence: [`research.md`](research.md) · Seams: [`contracts/internal-seams.md`](contracts/internal-seams.md)

## Approach in one paragraph

The shared bare store is the only place in the product that resolves a target
branch as `refs/heads/<name>` literally. Everywhere else — the validation that
accepted the user's choice, and the branch creation that consumes it — resolves
local-then-remote via `GitService::find_branch`. Make the store agree, and give
it the refs that agreement needs by mirroring the registered checkout's
`refs/remotes/*` alongside its `refs/heads/*`. Then fix the recovery fetch so it
is capable of succeeding, and stop the one failure this produces from arriving as
an unattributable internal error. Four small changes, one behaviour each, no new
concepts.

## No data model changes

No migration, no new table or column, no wire type. `WorkspacePlacement` and its
state machine are untouched; the change lives entirely between "the store exists"
and "the worktree is created". `shared/types.ts` does not regenerate
(`ApiError` is `#[ts(type = "string")]`).

## Steps

### Step 1 — `GitCli::fetch_with_refspecs` (seam S1)

`crates/git/src/cli.rs`

Generalise `fetch_with_refspec` over the refspec count; keep the single-refspec
function as a delegation so its three callers are untouched. One invocation
rather than two keeps one process spawn and one connection inside the
administration lease.

### Step 2 — resolve local-then-remote (seam S2)

`crates/workspace-manager/src/shared_repository.rs`

Add `resolved_branch_ref`, reimplement `branch_commit_present` on it, and have
`ensure` log which ref form it resolved to before acting. Presence keeps being
proven with `commit_exists`, and both ref forms keep `commit_exists`'s single
fail direction.

*Constitution XXI:* the new resolver's doc comment names
`GitService::find_branch` as the rule it matches, so the next reader does not
have to rediscover the relationship.

### Step 3 — mirror the checkout's remote-tracking refs (seam S4)

`crates/workspace-manager/src/shared_repository.rs`, `publish_and_fetch`

Fetch both refspecs, best-effort, still inside the lease and still after
`configure()` and the rename. Additive; no `--prune`.

Without this, Step 2 alone moves the failure from `ensure` into
`GitService::create_branch` and changes the message rather than the outcome
(`research.md` §5).

### Step 4 — a recovery fetch that can succeed (seam S3)

`crates/workspace-manager/src/shared_repository.rs`, `publish_and_fetch`

Extract `fallback_refspec`, and break out of the remotes loop on the branch being
*present* afterwards rather than on `git fetch` exiting zero. This removes the
guaranteed-to-fail forge round trip that is the user-visible stall
(`research.md` §7).

### Step 5 — surface the failure (seam S5)

`workspace_manager.rs` → `container.rs` (services) → `container.rs`
(local-deployment) → `error.rs`.

One variant per layer, one new match arm, scoped to this single failure. 500 is
retained so the server-side `tracing::error!` record survives
([`clarifications.md`](clarifications.md) C4).

### Step 6 — tests

In `crates/workspace-manager/src/shared_repository.rs`'s existing `mod tests`,
reusing `seed_repo` / `repo_record`, plus a `store_with_locks` helper built on
the in-memory `repository_admin_locks` pool pattern from
`crates/worktree-manager/src/worktree_manager.rs:45-66` (`ensure`, unlike
`adopt`, takes the lease).

The fixture builds a "checkout" that has a real `refs/remotes/origin/main`, so it
mirrors what `/srv/src/<repo>` actually looks like rather than what is convenient.

1. `ensure_serves_a_remote_prefixed_target_branch` — AC 1 and 2.
2. `branch_resolution_prefers_a_local_branch_over_a_remote_one` — AC 3.
3. `fallback_refspec_targets_the_remote_tracking_namespace` — AC 5, truth table.
4. `ensure_reports_which_repository_and_branch_it_could_not_serve` — AC 6.

The existing `commit_presence_is_proven_not_assumed` stays green unchanged and
pins FR-9/AC 4.

### Step 7 — verify

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
`pnpm run format`. Confirm the new tests fail against unmodified `main` first —
"a retention test that also passes before the fix proves nothing".

Do **not** run a dev server on this host: `cleanup_orphan_workspaces` always
sweeps the default base dir and, against a non-matching DB, would classify the
live cluster workspaces on the share as orphans.

## Constitution check

| Principle | How this plan honours it |
| --- | --- |
| III small, reversible steps | Four behaviour changes in one crate plus a three-line error channel. No new machinery; the store, the lease, and the placement state machine are untouched. |
| VI don't rebuild what shipped | Extends `SharedRepositoryStore` and `GitCli` rather than adding a parallel resolver; `fetch_with_refspec` is generalised, not duplicated. |
| XI diagnostics are evidence | The backend text reaches the user verbatim; nothing truncates or reinterprets it. |
| XVIII distributed execution | Ref mirroring stays coordinator-only and inside the per-repository fenced lease. |
| XX cross-node paths | Presence is proven with `cat-file -e`, never `rev-parse`; mirroring is additive so a repository-wide namespace shared by every node is not pruned. |
| XXI one convention per concept | The store now resolves target branches by the same rule as `GitService::find_branch`, and accepts the producer's default (`origin/main`) rather than only the non-default case. The failure carries the repository and branch that identify it, and the widening is scoped to that one failure. |
| Constraints | No new dependency; no generated file edited; `pnpm run format` in Step 7. |

No conflicts. No exceptions requested.

## Risks

| Risk | Mitigation |
| --- | --- |
| Mirroring `refs/remotes/*` grows the store | Refs only, no new objects beyond those already fetched by the heads mirror; `git clone --bare` created none, so there is nothing to conflict with. |
| A store created before this change lacks the remote refs | `ensure` runs on every provisioning and is idempotent; the first cluster workspace created for that repository after the fix backfills them. No migration needed. |
| Broadening resolution weakens `adopt`'s refusal | `adopt` always resolves a workspace branch (`vk/…`), a local name, so the remote arm cannot fire for it. Its existing refusal test stays unchanged as the pin. |
| The new error message leaks a path | It carries repository name, branch name and a git message. The store path is already in the current text; no environment values or credentials are added. |
