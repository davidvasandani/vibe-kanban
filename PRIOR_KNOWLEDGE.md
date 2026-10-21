# Prior knowledge — vk/37ef-fix-tables

Distilled from the knowledge base (`wiki/`). The knowledge base was only
read, not modified. No page covers markdown tables or `TABLE_TRANSFORMER`
directly; the relevant knowledge is about the shared Lexical markdown
surface they live in.

## Directly relevant

- **read-only-markdown-links** (`vk/e4ef-urls-always-clic`):
  - Every `WYSIWYGEditor` with `disabled` (conversation entries, issue
    descriptions, comments, approvals, notes) is read-only markdown, built
    from the same transformer list. A transformer fix applies to all of them.
  - Read-only editors still export through `MarkdownSyncPlugin`'s
    `onChange` (issue descriptions wire it to `onFormChange`). So the table
    export must re-import to the same table, or a display can rewrite stored
    content. The import → export → import round trip should be tested.
  - Tests for Lexical behaviour live in `packages/web-core` (vitest) and
    import from `@vibe/ui/...`; `packages/ui` has no test runner.
  - `packages/ui` formats with
    `--config ../../packages/local-web/.prettierrc.json`; a bare prettier
    call rewrites every line to double quotes.
- **frontend-linting**:
  - No ESLint directive comments are allowed at all.
  - `.ts` (non-component) file names are free-form; `.tsx` must be
    PascalCase. A plain `.test.ts` avoids that rule.
  - Web-core tests are linted through `tsconfig.eslint.json`.

## Not found

- No prior page on Lexical version upgrades. This task shows the
  `@lexical/markdown` 0.36 import contract changed underneath the custom
  transformer (see SPEC.md root cause), which is worth recording afterwards.
