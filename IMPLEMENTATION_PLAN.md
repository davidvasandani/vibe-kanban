# Implementation Plan: Search by Issue ID

1. Inspect the current remote global-search SQL, result contract, frontend
   aggregation/rendering, route construction, and focused test harnesses.
2. Add membership-scoped issue rows to remote global search, matching the
   human-readable `simple_id`, returning the issue UUID and owning project UUID,
   and participating in the existing per-category cap/truncation behavior.
3. Extend the frontend result kind and Global Search dialog with an Issues
   group whose result text exposes both `simple_id` and title and whose selection
   uses the existing organization coordination and issue-detail route builder.
4. Add regression coverage for case-insensitive issue-ID matching,
   authorization boundaries, result limits, issue rendering, organization
   selection, and issue-detail navigation.
5. Run focused frontend and backend/SQL tests, then repository formatting and
   proportionate type/lint checks; address regressions.
6. Run the required independent Codex diff review and iterate until it has no
   significant findings.
7. Update the Global Search knowledge-base page and index with reusable lessons,
   tag it with `vk/b1df-searching-by-iss`, and commit the knowledge-base update.
8. Commit the implementation, push the task branch, open a pull request against
   the repository base branch, wait for required checks as needed, and merge it.
