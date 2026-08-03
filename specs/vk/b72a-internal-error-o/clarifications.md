# Clarifications — `b72a-internal-error-o`

Resolutions for the `[NEEDS CLARIFICATION]` markers in [`spec.md`](spec.md).
Each records the decision, the reasoning, and what was rejected.

## C1 — Is mirroring the checkout's remote-tracking refs best-effort or fatal?

**Best-effort, exactly like the existing heads mirror.**

The heads mirror is already tolerant (`if let Err(e) = … { debug!(…) }`) and the
comment above it states the rule: *"A remote that cannot be reached is
tolerated; the target branch resolving afterwards is not optional."* The gate is
the closing presence check, not the fetch's exit status. Making the remotes
mirror fatal would introduce a second, stricter rule for a fetch that is
frequently a no-op — a store that already carries the branch does not need it —
and would fail provisioning for a repository whose checkout is momentarily
unreadable even when the branch is already present.

Verified empirically that this does not weaken the heads mirror: git exits 0
when a wildcard refspec matches nothing, so the two refspecs can share one
invocation without the remotes half being able to fail the heads half.

```
$ git -C store fetch <src> '+refs/heads/*:refs/heads/*' '+refs/remotes/*:refs/remotes/*'
# exit 0 even when <src> has no refs/remotes at all
```

**Rejected:** two separate fetch invocations. One invocation is one process spawn
and one connection inside the administration lease, and the no-match tolerance
removes the only reason to split them.

## C2 — How fresh must the store's view of `origin/main` be?

**Exactly as fresh as the registered checkout, and no fresher. No new network
fetch is introduced to freshen it.**

The branch picker offers what `get_all_branches` reads from that same checkout,
so mirroring the checkout's `refs/remotes/*` makes the set of branches a user can
*pick* and the set the store can *serve* the same set, by construction. Any
staleness is staleness the user was already shown. Adding a freshening fetch
would put a network round trip on the create path to fix a discrepancy that
cannot arise.

This is also the safe direction against a recorded homelab hazard: cloning from
a local checkout and then trusting the *clone's own* `origin` resolves
`origin/main` to the intermediate clone's local `main`, not the forge's. Copying
the checkout's remote-tracking refs verbatim avoids inventing a second,
differently-fresh `origin/main`. `git clone --bare` creates no `refs/remotes/*`
at all, so there is nothing stale in the store to shadow the mirrored refs.

## C3 — Does the forge recovery fetch survive?

**Yes, kept and corrected — not deleted.**

The checkout mirror covers the dominant case, but not the case the recovery loop
was written for: a branch that exists upstream and that the coordinator's
checkout has not fetched. That case is real for plain local branch names too, so
deleting the loop would be a behaviour regression beyond this feature's scope.

Two corrections, both required for it to be capable of succeeding at all:

1. the refspec must name the branch as *upstream* sees it and land it where the
   resolver will look — for target `origin/main` and remote `origin`, that is
   `+refs/heads/main:refs/remotes/origin/main`, not
   `+refs/heads/origin/main:refs/heads/origin/main`;
2. the loop stops when the branch is afterwards *present*, not when `git fetch`
   exits zero. A zero exit is not evidence, per the constitution's
   "an object a path claims to reference is proven present, never assumed".

## C4 — What HTTP status does a shared-store provisioning failure carry?

**500, with a message naming the repository, the branch, and what failed.**

The caller did not supply bad input: the branch it sent is one this same API
offered it, and by default it is the value the API's own picker chose. The
inability to serve it is the server's provisioning failure, so a 4xx would
misattribute it.

The decisive argument is operational rather than semantic. `error.rs` logs via
`tracing::error!` only when `info.status.is_server_error()`. Downgrading this to
a 4xx would remove the server-side record of the exact failure this task exists
to make visible — the opposite of the intent. The shape to copy is
`ApiError::Worktree`, already a 500 that carries a real message.

**Rejected:** the typed serde-tagged payload used by browser-session errors.
That pattern exists because MCP tools and the frontend parse those errors back;
nothing parses a provisioning failure, and a human reads it. Adding a wire
contract with no consumer would be speculative generality.

## Spec updates applied

- FR-4 now states the mirror is best-effort and that the presence check is the
  gate (C1).
- FR-6 now states the recovery fetch is retained in a corrected form and judged
  by presence (C3).
- FR-7 now fixes the status as 500 with an attributed message (C4).
- A new non-goal records that no freshening fetch is added (C2).
- All four `[NEEDS CLARIFICATION]` markers removed.

## Still open

None.
