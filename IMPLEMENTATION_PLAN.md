# Fix Toolbar — Implementation Plan

1. Confirm the mobile navbar's current rendered structure and existing tests,
   including the split between the flexible workspace-tool region and trailing
   status/actions.
2. Establish/refine the SpecKit project constitution and generate the task's
   feature artifacts through the required specify, clarify, plan, tasks, and
   analyze stages.
3. Update the shared `Navbar` mobile workspace layout so:
   - the leading toolbar region grows and may shrink (`flex-1 min-w-0`);
   - the visible tab group fills that region;
   - each visible tab shares surplus width while retaining a practical minimum;
   - horizontal overflow remains available when space is constrained; and
   - the trailing action region stays non-shrinking.
4. Add focused rendered-component tests in the web-core Vitest lane that assert
   the flex growth/distribution/overflow contract, active accessibility state,
   and fixed trailing controls without changing project-page behavior.
5. Run focused tests, TypeScript checks, lint, repository formatting, and a
   scoped diff sanity check.
6. Run an independent Codex diff review; address and re-verify all confirmed
   significant findings until the review is clean.
7. Distill reusable responsive-toolbar knowledge into the Vibe Kanban knowledge
   base, tag it with `vk/2163-fix-toolbar`, refresh the index, and commit the
   knowledge-base update.
8. Commit the implementation, push the task branch, open a pull request against
   the repository's base branch, wait for required checks as needed, and merge
   it.
