# Implementation Plan: Mobile Workspace Floating Context Bar

**Task ID:** `vk/2792-vk-workspace-flo`
**Inputs:** `SPEC.md`, `PRIOR_KNOWLEDGE.md`

## Objective

Prevent the desktop workspace context bar from rendering in the responsive
mobile workspace layout while preserving all desktop behavior.

## Steps

1. **Establish SpecKit artifacts**
   - Refresh the repository constitution.
   - Generate the feature specification at a task-specific path.
   - Clarify open questions, with the default decision that the redundant
     context bar is hidden rather than made touch-draggable.
   - Generate the technical plan, supporting research/data-model/contracts,
     dependency-ordered tasks, and analysis report.

2. **Create a testable visibility rule**
   - Add a small pure helper near the context-bar container that accepts the
     responsive-mobile and physical-mobile signals.
   - Make the rule return hidden when either signal reports mobile.
   - Add focused tests for:
     - responsive mobile / unrecognized physical device;
     - physical mobile / desktop-sized responsive state;
     - normal desktop rendering.

3. **Apply the responsive guard**
   - Read `useIsMobile()` unconditionally in `ContextBarContainer`.
   - Combine it with the existing `isRealMobileDevice()` guard.
   - Return `null` before rendering the presentational `ContextBar` when the
     combined rule says it is mobile.
   - Leave action preparation, desktop positioning, mouse dragging, and
     persisted snap position unchanged.

4. **Verify the change**
   - Install dependencies if the fresh worktree requires it.
   - Run focused context-bar tests.
   - Run the relevant frontend type check and ESLint target.
   - Run repository formatting and confirm it does not alter unrelated files.
   - Inspect the final diff for scope and accidental generated-file changes.

5. **Independent review**
   - Run the requested Codex diff-review workflow. If the named
     `codex-review` skill is unavailable, use the repository's Codex CLI as
     the closest independent review mechanism and record that fallback.
   - Address confirmed significant findings.
   - Repeat focused verification and review until there are no significant
     findings.

6. **Record reusable knowledge**
   - Add or update a single knowledge-base topic describing the responsive
     versus physical-device visibility rule and why mobile reuses the navbar
     instead of the desktop floating context bar.
   - Tag it with `vk/2792-vk-workspace-flo`, refresh `wiki/INDEX.md`, and
     commit the knowledge-base update as required by the pipeline.

## Expected Files

- `packages/web-core/src/pages/workspaces/ContextBarContainer.tsx`
- A focused test file beside the context-bar visibility helper/container
- `specs/...` SpecKit artifacts
- `SPEC.md`
- `PRIOR_KNOWLEDGE.md`
- `IMPLEMENTATION_PLAN.md`
- `wiki/INDEX.md`
- One relevant `wiki/*.md` topic page

## Ordering and Parallelism

- SpecKit stages are strictly sequential.
- The helper test and container integration share the same small seam and
  should be implemented in dependency order.
- Verification commands that do not mutate files may run together after
  formatting.
- Independent review starts only after implementation verification.
- Knowledge-base writes happen only after the shipped behavior is settled.

## Rollback

The functional change is isolated to a render guard. Reverting the helper,
test, and combined guard restores the prior context-bar visibility without
data migration or persistence cleanup.
