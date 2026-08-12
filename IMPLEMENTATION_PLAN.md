# Implementation Plan: Fix Right Drawer Section Spacing

1. Refresh the SpecKit constitution and generate the task-specific SpecKit
   specification, clarification, plan, task list, and analysis artifacts.
2. Extend `RightSidebar` section metadata with an explicit sizing policy so
   each section decides whether its expanded body fills available height.
3. Configure Server Affinity as intrinsic-height and retain flexible sizing for
   content panels that grow or scroll.
4. Add a focused `RightSidebar` regression test that renders the section
   composition and proves expanded Server Affinity is not `flex-1`, while a
   fillable section still is.
5. Run focused Vitest coverage, TypeScript checks, lint, repository formatting,
   and whitespace validation.
6. Execute an independent Codex diff review; address confirmed significant
   findings and repeat verification/review until clean.
7. Update the project knowledge base and index with the reusable per-section
   sizing rule, tagged with task `d80e-fix-the-spacing`, and commit it.
8. Commit the implementation intentionally, push the task branch, open a pull
   request against the repository base branch, wait for required checks, and
   merge the pull request.

## Dependency Order

- Steps 2 and 3 depend on the completed SpecKit artifacts.
- Step 4 depends on the final composition API from steps 2 and 3.
- Step 5 depends on implementation and tests.
- Step 6 depends on a verified diff; any review fix loops back through step 5.
- Step 7 depends on the final shipped behavior.
- Step 8 depends on all local verification, review, and documentation work.
