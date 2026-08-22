# Implementation Plan: Repository-scoped Git panel PR state

1. Inspect the current Git panel transformation, branch-status query contract,
   and test/tooling conventions.
2. Establish the SpecKit constitution and produce the feature artifacts through
   clarify, plan, tasks, and analyze.
3. Extract a pure `RepoWithTargetBranch[]` + `RepoBranchStatus[]` to
   `RepoInfo[]` transformation that joins only by `repo_id`.
4. Remove the unscoped workspace-summary fallback and now-unused workspace
   context dependencies from `GitPanelContainer`.
5. Add focused unit tests for one-PR/multiple-repo state, unloaded status,
   open-over-merged precedence, and ordinary branch metadata.
6. Install locked dependencies if needed, format the repository, and run the
   focused test plus frontend type/lint verification in proportion to the
   change.
7. Run an independent Codex diff review; fix confirmed significant findings and
   repeat verification/review until clean.
8. Add the repository-association invariant to the project knowledge base,
   refresh its index, and commit the knowledge-base update.
9. Commit all implementation artifacts, push the task branch, open a PR against
   the repository's base branch, monitor required checks, and merge it.
