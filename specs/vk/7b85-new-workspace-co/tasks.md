# Tasks: New workspace config never falls below the fold

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Setup
- [x] T001 Install workspace dependencies for this fresh worktree: `pnpm install --frozen-lockfile` at the repo root (prerequisite for every check below; no files changed)

## Phase 2: Shared presentational layer (`packages/ui`)
- [x] T002 Add the optional `fillHeight` prop to `ChatBoxBaseProps` and its destructured default in `packages/ui/src/components/ChatBoxBase.tsx` (depends on T001)
- [x] T003 Apply the containment contract in `packages/ui/src/components/ChatBoxBase.tsx`: shrink-only `min-h-0` (not `flex-1`) on the root and on the editor area when filling; `shrink-0` on the error alert, the fill-mode banner box and the header block; `shrink-0` on the footer controls row (depends on T002)
- [x] T004 Wrap the injected `editor` node in an unconditional slot `div` in `packages/ui/src/components/ChatBoxBase.tsx` carrying `min-h-0 overflow-y-auto` only when filling, so the flex-child count and `gap-plusfifty` are identical in both modes (depends on T003)
- [x] T005 Add the stable `data-testid`s `chat-box-editor-slot` and `chat-box-footer` in `packages/ui/src/components/ChatBoxBase.tsx` per `./contracts/layout-contract.md` §4 (depends on T004)
- [x] T006 Accept `fillHeight` in `CreateChatBoxProps` and forward it to `ChatBoxBase` in `packages/ui/src/components/CreateChatBox.tsx` (depends on T002)

## Phase 3: Create-mode container (`packages/web-core`)
- [x] T007 Declare the shell and column contract in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`: add `min-h-0 overflow-y-auto` to the root (the shell owns overflow and scrolls rather than clips, so a viewport below the supported range leaves the create action reachable), `min-h-0` to the centring row, and `max-h-full min-h-0` to the content column (depends on T001)
- [x] T008 Mark the fixed rows `shrink-0` in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`: linked-issue warning, both step headings, and the placement ("Run on") row (depends on T007)
- [x] T009 Add `min-h-0` to the `@container` wrapper and the inner composer column in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`, so no intermediate automatic minimum stalls the deficit above the editor slot (depends on T007)
- [x] T010 Give the repository-selection step its own `min-h-0` and vertical overflow in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`, satisfying FR-6 (depends on T007)
- [x] T011 Pass `fillHeight` to `CreateChatBox` and replace the editor class `min-h-double max-h-[50vh] overflow-y-auto` with `min-h-double` in `packages/web-core/src/shared/components/CreateChatBoxContainer.tsx` (depends on T006, T009)

## Phase 4: Regression coverage
- [x] T012 Add `packages/remote-web/src/test/CreateChatBox.test.tsx` rendering the real `CreateChatBox`, asserting in fill mode: root shrinkable; editor slot has a zero minimum and owns vertical overflow; footer config row is `shrink-0` (depends on T005, T006)
- [x] T013 Extend `packages/remote-web/src/test/CreateChatBox.test.tsx` with the containment assertion: the injected editor is a descendant of the editor slot while the footer config row is not (depends on T012)
- [x] T014 Extend `packages/remote-web/src/test/CreateChatBox.test.tsx` with the FR-5 cases: a validation error and the in-flight create state each leave the footer present and `shrink-0` (depends on T012)
- [x] T015 Extend `packages/remote-web/src/test/CreateChatBox.test.tsx` with the FR-8 case: with `fillHeight` omitted, none of the fill-mode classes are present (depends on T012)

## Phase 5: Validation
- [x] T016 Run the focused suite: `pnpm --filter @vibe/remote-web exec vitest run src/test/CreateChatBox.test.tsx src/test/SessionChatBox.test.tsx` — the new contract passes and the existing `SessionChatBox` coverage is unbroken (depends on T011, T013, T014, T015)
- [x] T017 Run `pnpm run check` (frontend + all backend Rust workspaces) (depends on T011, T015)
- [x] T018 Run `pnpm run lint` (serial with T017: both invoke cargo over the same workspaces and contend on the build lock) (depends on T011, T015)
- [x] T019 Run `pnpm run format`, then `git diff --check` (depends on T016, T017, T018)
- [!] T020 **NOT DONE — no browser runner available.** Browser verification of the pixel-level outcome — JSDOM proves nothing about layout. Cover, and record evidence for, each case or record explicitly that no browser runner was available (depends on T016):
  - T020a narrow viewport + long prompt: config row and create action fully visible, prompt scrolls internally (FR-1, FR-2);
  - T020b narrow viewport + short prompt: unchanged from today, still vertically centred (FR-7);
  - T020c the repository-selection step at the same narrow viewport (FR-6);
  - T020d all three hosts — mobile chat tab, desktop workspaces left panel (`h-full overflow-hidden`), project sidebar Create Workspace panel (`flex-1 min-h-0`) — since the two height chains differ (FR-9).

## Phase 6: Close-out
- [x] T021 Independent review of the task diff; address confirmed findings and re-verify (depends on T019, T020)
  - Codex CLI was **unavailable**: `~/.codex/auth.json` holds an empty `refresh_token`, so every `codex exec` attempt fails token refresh with `400 invalid_request_error`. Re-authenticating Codex is the operator's call, not something to do silently mid-task.
  - Substituted the repository's `/code-review` skill at high effort, run three times: initial, after the first round of fixes, and after the second.
  - Round 1 findings, both fixed: the shell clipped with every row `shrink-0` and no scroll fallback (now `overflow-y-auto` as a last resort); the always-rendered `shrink-0` wrapper around the duplicate-workspace advisory left an empty flex child and a stray `gap-base` whenever the advisory rendered `null` (now a `className` on the advisory's own root, with regression coverage).
  - Round 2 findings, both fixed: the checked-in contract still specified `flex-1` where the code ships shrink-only sizing (docs corrected, with the reason recorded); the `fillHeight` doc claimed the banner stopped shrinking but `{banner}` was bare (now boxed `shrink-0` in fill mode, with coverage).
  - Round 3 findings: the editor slot could collapse to zero behind a tall advisory (now floored at `3rem`); "the shell clips" wording in the constitution and T007 contradicted the shipped `overflow-y-auto`; `IMPLEMENTATION_PLAN.md` still prescribed `flex-1`; the plan promised a container clipping-class test that was never written. All corrected. Round 3 confirmed no correctness bug in the flex chain itself.
- [x] T022 Update `docs/knowledge-base/nested-flex-scroll-containment.md` and `docs/knowledge-base/INDEX.md` with this task's tag and the create-composer application (depends on T021)
- [x] T023 Open the pull request against the base branch and merge it (depends on T021, T022) — PR #315, squash-merged into `main` after a rebase that resolved a constitution version-number collision (`main` had also claimed 0.32.0; this change restacked as 0.33.0).

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
