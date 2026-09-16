# Implementation plan — vk/fe2d-warn-when-starti

1. Complete SpecKit constitution, feature specification, clarification, technical plan, tasks and analysis in pipeline order. Refresh stale task paths in the checked-in command templates.
2. Reuse project-scoped synced workspaces and PRs for exact issue matching. Join available local workspace metadata by local workspace ID; retain remote-only siblings with explicit unknown branch/status.
3. Add a shared presentational advisory and create-mode container. Show it before submit, allow dismissal and existing-workspace navigation, preserve the existing creation mutation and submit guard.
4. Add a plural active-workspace badge to the issue workspace section header, visible while collapsed.
5. Add focused projection and rendered-component regressions for filtering, identity, evidence, dismissal, navigation and passive visibility. Install frozen dependencies, run checks and formatting.
6. Run independent Codex CLI review, resolve significant findings and reverify.
7. Record reusable knowledge, update index, commit, open PR against main and merge after required checks.
