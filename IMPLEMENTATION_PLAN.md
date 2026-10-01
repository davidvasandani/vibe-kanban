# Implementation plan: lint web-core and remote-web

Task `vk/848f-lint-packages-we`. See `SPEC.md` for the design,
`PRIOR_KNOWLEDGE.md` for rules inherited from earlier tasks, and
`specs/vk/848f-lint-packages-we/plan.md` §7 for the reasoning behind each
hook fix.

## Steps

1. **Extract the shared rule set.** Move local-web's `.eslintrc.cjs` body
   into `eslint.frontend.cjs` as `createFrontendConfig({ project,
   ignorePatterns })`. Reduce local-web's config to a call into it. Hash
   `eslint --print-config` for sample local-web files before and after: the
   hashes must match.
2. **Wire web-core.** Add `.eslintrc.cjs`, a `tsconfig.eslint.json` that
   includes tests and `*.config.ts`, and a `lint` script.
3. **Wire remote-web.** Same files as web-core, plus local-web's ESLint
   devDependencies at the same versions. Run `pnpm install` to update
   `pnpm-lock.yaml`.
4. **Root and CI.** Add `web-core:lint` and `remote-web:lint`, and append
   both to `lint`. In `frontend-checks`, add `core:lint` and `remote:lint`
   to the job, and add `eslint.frontend.cjs` to the path filter.
5. **Naming rule.** PascalCase for `src/**/!(use*).tsx`, camelCase for
   `src/**/use*.{ts,tsx}`. Rename `settingsRegistry.tsx` to
   `SettingsRegistry.tsx`.
6. **Layer moves.** Use `git mv` and change only import paths:
   `createModeSeedStore` → `shared/stores`, the create-mode chat box
   containers → `features/create-mode/ui`, and `SharedAppLayout` →
   `pages/root`. Same-feature imports become relative. Typecheck all three
   apps.
7. **Code findings.** Fix the unused variables (copy plus `delete`, drop
   unused parameters), add the `no-empty` comment, remove the directives and
   type `rehypePlugins`, add the switch cases, and delete the barrel.
8. **`exhaustive-deps`.** Work one site at a time, as in plan §7. Re-lint
   after each group until both packages report 0 problems.
9. **Verify.** `pnpm run lint`, `pnpm run check`, `pnpm run format`, the
   web-core and remote-web vitest suites, and a negative proof (add an
   unused import, lint exits 1, revert).
10. **Docs.** Update the `AGENTS.md` lint line, the wiki "Verification
    notes", and the stale `settingsRegistry.tsx` mentions.
11. **Review, knowledge base, PR.** Codex review until clean, a wiki page on
    frontend linting, then open and merge the PR.

## Risks

- **Hook dependency edits change behaviour.** Each site is reasoned through
  in plan §7, and the vitest suites cover the conversation-history and
  settings code paths.
- **File moves break imports that tsc can't see,** such as `vi.mock`
  specifiers. Vitest mocks by resolved module id, so a component that
  imports its dependency relatively is still mocked by an aliased
  specifier. The remote-web test suite confirms this.
- **Lockfile churn.** pnpm also deduped `debug` and `acorn` within their
  existing ranges. No new packages were added.
