import type { GitBranch } from 'shared/types';

/**
 * Built-in preference for the default target branch, in priority order, when a
 * repository has no explicitly configured default. `origin/main` is the
 * requested default; `origin/master` is the classic main/master naming
 * fallback so `master`-based repos benefit too.
 */
export const DEFAULT_BRANCH_PREFERENCE = ['origin/main', 'origin/master'];

/**
 * Choose the branch a repository should default to when it is added on the
 * create-issue screen, so the common case (start from the remote mainline)
 * needs no manual pick.
 *
 * Priority (first that exists in `branches`, matched by exact name):
 *   1. `preferredBranch` — the repo's configured `default_target_branch`, so an
 *      explicit user choice outranks the built-in default.
 *   2. `origin/main`, then `origin/master` (`DEFAULT_BRANCH_PREFERENCE`).
 *   3. the current branch (`is_current`).
 *   4. the first available branch.
 * Returns `null` only when `branches` is empty.
 */
export function resolveDefaultBranch(
  branches: GitBranch[],
  preferredBranch?: string | null
): string | null {
  const byName = (name: string | null | undefined): string | undefined =>
    name ? branches.find((branch) => branch.name === name)?.name : undefined;

  const preferred =
    byName(preferredBranch) ??
    DEFAULT_BRANCH_PREFERENCE.map(byName).find((name) => name !== undefined);
  if (preferred) return preferred;

  const current = branches.find((branch) => branch.is_current);
  return current?.name ?? branches[0]?.name ?? null;
}
