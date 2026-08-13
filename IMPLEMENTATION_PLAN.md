# Implementation Plan: Single-Value Browser Titles

1. Refresh the SpecKit constitution and capture any project-wide constraints
   that govern this frontend change.
2. Generate the SpecKit feature specification for single-value browser titles,
   using the task's concise title without concatenating a ticket number into
   the feature name.
3. Clarify selection precedence, empty-value behavior, and the boundary between
   browser metadata and visible breadcrumbs.
4. Generate the SpecKit technical plan, research, and any applicable contracts.
5. Generate dependency-ordered tasks and analyze all SpecKit artifacts for
   omissions or constitution violations.
6. Install frontend dependencies using the repository's frozen lockfile.
7. Add focused hook tests that demonstrate the current concatenation failure
   and cover single-title, fallback, blank-value, and rerender behavior.
8. Change `usePageTitle` to select the first non-empty title candidate and use
   `Vibe Kanban` only when no candidate exists.
9. Keep the issue title and project name as an ordered fallback chain on the
   kanban page; verify workspace call sites remain correct without changing
   visible breadcrumbs or ticket identifiers.
10. Run focused tests, formatting, frontend type checks, and linting appropriate
    to the affected packages; resolve any regressions.
11. Execute an independent Codex diff review, address confirmed findings, and
    repeat review and verification until no significant findings remain.
12. Record the reusable browser-title selection contract in the project wiki,
    tag it with this task id, refresh the wiki index, and commit the knowledge
    base.
13. Commit the implementation, push the task branch, open a pull request against
    the repository base branch, wait for required checks, and merge it.
