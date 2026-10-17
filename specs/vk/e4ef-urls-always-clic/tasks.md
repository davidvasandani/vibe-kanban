# Tasks: URLs always clickable

**Plan**: `./plan.md`

Tasks are ordered by dependency. Tasks marked **[P]** touch independent files
and may run in parallel within their group. Each task names the file(s) it
changes.

## Phase 1: Core
- [x] T001 [P] Add `findUrlMatches` and `ReadOnlyAutoLinkPlugin`. The
      linking pass (a tagged update) splits simple text into `AutoLinkNode`s, skips links
      and `CodeNode`, and unwraps on unmount. `MarkdownSyncPlugin` skips the tag.
      Paths: `packages/ui/src/components/ReadOnlyAutoLinkPlugin.tsx` (new),
      `packages/ui/src/components/MarkdownSyncPlugin.tsx`.
- [x] T002 [P] Make `http://` clickable and register the mutation listener for
      `AutoLinkNode` too. Path: `packages/ui/src/components/ReadOnlyLinkPlugin.tsx`.

## Phase 2: Wiring
- [x] T003 Register `AutoLinkNode` and mount `ReadOnlyAutoLinkPlugin` when
      `disabled`. Path: `packages/web-core/src/shared/components/WYSIWYGEditor.tsx`.
      Depends on T001.

## Phase 3: Validation
- [x] T004 [P] Add helper tables and rendered-DOM tests for every acceptance
      criterion, including the export round-trip. Path:
      `packages/web-core/src/shared/components/ReadOnlyAutoLinkPlugin.test.tsx` (new).
      Depends on T001 and T002.
- [x] T005 [P] Update the `http` expectations to clickable. Path:
      `packages/web-core/src/shared/components/ReadOnlyLinkPlugin.test.tsx`.
      Depends on T002.
- [x] T006 Run vitest for web-core, `pnpm run check`, `pnpm run lint`, and
      `pnpm run format`, and fix anything they report. Depends on T003, T004
      and T005.

<!--
Conventions:
- `T001` … task ids are stable and referenced by the dependency graph.
- `[P]` … parallel-safe (independent files). Omit for tasks that must be serial.
- `[ ]` / `[x]` … completion checkbox, toggled from the workbench.
-->
