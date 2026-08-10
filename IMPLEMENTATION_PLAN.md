# Implementation plan: default workspace bases to remote mainline

1. Reuse the existing `resolveDefaultBranch` policy in every repository branch
   selection hook used to assemble new-workspace inputs.
2. Preserve explicit initial-branch precedence, then delegate repository
   default and fallback selection to the canonical helper.
3. Add focused hook tests proving `origin/main`/`origin/master` outrank a current
   deployment branch while configured and explicit initial choices still win.
4. Retain exact remote-prefixed target branch names through workspace input
   creation; make no `/srv/src` checkout or deployment changes.
5. Run formatting, frontend tests, type checking, and linting.
6. Run SpecKit analysis and implementation tasks, then independent Codex review
   until no significant findings remain.
7. Update the existing branch-defaulting knowledge page and index contribution
   metadata, commit the knowledge update, and merge into the base branch.
