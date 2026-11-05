# Implementation plan: VAS-982

1. **`crates/git/src/cli.rs`**: add `GitCli::is_ancestor(repo_path, ancestor,
   descendant) -> Result<bool, GitCliError>` next to `merge_base`, wrapping
   `git merge-base --is-ancestor` with the same "non-zero exit is a plain
   `false`, not an error" convention `commit_exists` already uses. — Done.

2. **`crates/workspace-manager/src/shared_repository.rs`**:
   - Update `ensure`'s cheap-path branch to call a new
     `Self::fast_forward_local_head(&cli, &store, &repo.name, target_branch)?`
     after `store_resolves` returns `true`, before `mirror_branch_back`.
   - Add `fast_forward_local_head`: resolve the store's `origin` URL (no-op if
     absent), fetch `+refs/heads/<branch>:refs/remotes/origin/<branch>` from
     it (warn-and-continue on fetch failure), compare tips, fast-forward via
     `update-ref` when the local head is a strict ancestor of the fetched
     tip, else return `WorkspaceError::SharedStore` with a message naming
     both commits.
   - Update the now-stale doc comment on `store_resolves` that described the
     local-head staleness as "pre-existing ... deliberately left alone" —
     point it at the new function instead.
   - Done.

3. **Tests** (same file, `mod tests`):
   - `ensure_fast_forwards_a_stale_local_target_branch_to_origin`: checkout +
     a separate "forge" repo cloned from it; advance the forge only; first
     `ensure` call captures the checkout's (stale) tip; second call, same
     branch, must land at the forge's tip. — Done.
   - `ensure_refuses_a_diverged_local_target_branch`: same setup, but the
     store's local head is advanced independently (via a throwaway clone of
     the store) before the forge is checked, producing a commit the forge
     does not have; `ensure` must error with a message containing
     "diverged", and must leave the local ref untouched. — Done.
   - Confirmed by inspection that every pre-existing test in this file calls
     `ensure` with an `origin/`-prefixed branch (never a bare local name with
     a real/fake `origin` configured), so none of them exercise the new
     fetch and none should change behavior.

4. **Verify**: `cargo test -p workspace-manager -p git --lib` (in progress).
   `cargo fmt` + `cargo clippy` for the touched crates before calling this
   done, per repo convention (`pnpm run format` / `pnpm run lint` cover the
   whole workspace but touching just these two crates is cheaper while
   iterating).

5. **Explicitly not doing** (see technical-spec.md "Non-fixes"): fixing the
   analogous staleness gap in the non-clustered `repo.path` path; threading
   real per-repo GitHub credentials through the new fetch; a periodic
   background fast-forward job. Each is either out of scope, already matches
   established precedent elsewhere in the same file, or strictly smaller in
   impact than the on-demand fix already delivered.

6. **Review and ship**: run the `codex-review` pass over the diff, address
   any confirmed findings, then open and merge the PR against `main`.
