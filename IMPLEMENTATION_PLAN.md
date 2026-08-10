# Implementation Plan: Desktop Deploy Status (`VAS-377`)

Companion to [`SPEC.md`](SPEC.md) and
[`PRIOR_KNOWLEDGE.md`](PRIOR_KNOWLEDGE.md). Steps are dependency ordered.

1. **Confirm the existing contract and composition boundary**
   - Trace `useUserSystem` deployment metadata and the shared `DeployStatus`
     behavior added by the mobile implementation.
   - Confirm the desktop workspace `RightSidebar` is the persistent drawer
     controlled by `ToggleRightSidebar` and preserve its flex/overflow contract.

2. **Add the fixed desktop deploy-status row**
   - Read `appVersion` and `deploymentTimestamp` from the existing user-system
     context inside the workspace right drawer.
   - Render a labelled, non-collapsible, intrinsic-height row above the current
     section list.
   - Reuse `DeployStatus`; add only an additive presentation option or class
     override if necessary for the wider desktop layout.
   - Do not add a preference key, visibility action, API request, backend field,
     or homelab change.

3. **Add regression coverage**
   - Cover drawer placement and the absence of a collapse/toggle affordance.
   - Verify production revision, `dev`, missing timestamp, and invalid timestamp
     rendering through the shared presentation tests.
   - Protect the intrinsic row and bounded drawer flex/scroll class contract.

4. **Run SpecKit implementation and verification**
   - Execute the dependency-ordered SpecKit tasks, ticking each item as it
     lands.
   - Run `pnpm install --frozen-lockfile` if the worktree is not prepared.
   - Run focused tests plus appropriate repository type checks, generated-type
     check, lint, formatting, and `git diff --check`.
   - Record verification results in the feature directory.

5. **Review, document, and integrate**
   - Run independent Codex CLI review of the task diff, address confirmed
     findings, and repeat until no significant findings remain.
   - Add the reusable desktop placement/layout lesson to the most relevant
     project knowledge page, tag it with `VAS-377`, refresh the index if needed,
     and commit the knowledge-base update.
   - Merge the task branch into its configured base branch only after the task
     diff is clean, reviewed, and limited to Vibe Kanban.
