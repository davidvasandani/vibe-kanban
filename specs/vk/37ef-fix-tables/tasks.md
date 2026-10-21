# Tasks: Markdown tables render as written

**Plan**: `specs/vk/37ef-fix-tables/plan.md`

## Layer 1 — fix and tests (independent files)

- [x] T001 [P] Fix divider detection (from `match[0]`, loosened regex, header
  only when the table has one row, stray divider stays text), keep empty
  cells, split on unescaped pipes, pad short rows, escape `|` on export.
  File: `packages/ui/src/lib/table-transformer.ts`. (FR-001…FR-006)
- [x] T002 [P] Add vitest coverage for AC-1…AC-6 through
  `[TABLE_TRANSFORMER, ...TRANSFORMERS]` in a headless Lexical editor,
  including import → export → import.
  File: `packages/web-core/src/shared/lib/tableTransformer.test.ts`.
- [x] T003 [P] Delete the throwaway reproduction test.
  File: `packages/web-core/src/shared/components/zz-repro.test.ts`.

## Layer 2 — verification (depends on T001–T003)

- [x] T004 Run `pnpm --filter @vibe/web-core test` (new tests pass, no
  regressions).
- [x] T005 Run `pnpm run check` and `pnpm run lint` (frontend parts; no Rust
  changed, so backend check/clippy were not re-run).
- [x] T006 Run `pnpm run format` and confirm only intended files changed.

## Dependencies

T001, T002, T003 → T004 → T005 → T006.
