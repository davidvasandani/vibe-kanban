# Frontend linting

How ESLint covers the frontend packages, and the traps in fixing what it
reports.

## Layout

- `pnpm run lint` runs ESLint for `local-web`, `web-core`, `remote-web` and
  `ui`, then clippy and the unused-i18n-key check. CI's `frontend-checks`
  runs the same package lints (`local:lint`, `core:lint`, `remote:lint`,
  `ui:lint`).
- The three app packages share one rule set: `eslint.frontend.cjs` at the
  repo root exports `createFrontendConfig({ project, ignorePatterns })`. Each
  package's `.eslintrc.cjs` only passes its tsconfig and its own ignores.
  `packages/ui` keeps its own, smaller config.
- The shared config is pulled in with `require` and spread into the
  package's config, not referenced through `extends`. As a result ESLint
  resolves plugins from the **consuming package's** `node_modules`. Every
  package that calls the factory must list the same ESLint plugins in its
  devDependencies, or it fails with "plugin not found".
- The CI path filter for `frontend-checks` lists `eslint.frontend.cjs`
  explicitly, because it lives outside `packages/`. Without that entry, a
  rules-only change would skip the lint job and read as a pass.

## Type-aware parsing must cover tests

`web-core/tsconfig.json` excludes `*.test.ts(x)` from `tsc`. With
`parserOptions.project` pointing at it, every test file becomes a parse
error ("TSConfig does not include this file"). Each package therefore lints
against `tsconfig.eslint.json`, which extends the main tsconfig, includes
`src` and `*.config.ts`, and clears `exclude`. `tsc --noEmit` is unaffected.
Don't "fix" those parse errors by adding test globs to `ignorePatterns`,
because that silently stops linting the tests.

## Rules that surprise people

- **No ESLint directive comments at all.** The rule is
  `eslint-comments/no-use` with `allow: []`. An "inline disable with a
  justification" is not available. A genuine exception goes in the shared
  config, scoped to the named file, with a comment explaining why.
- **Layer boundaries apply to web-core too.** The `src/shared`,
  `src/features` and `src/pages` overrides are path-relative, so they bind
  web-core's FSD layout exactly as they bind local-web's. Shared code may
  import only from shared. A feature may not import `@/features/**`,
  **including itself**, so same-feature imports must be relative. App shells
  that compose page containers (`SharedAppLayout`) belong in `pages/`, not
  `shared/`.
- **File names.** `.tsx` files are PascalCase unless they start with `use`.
  Hooks and their tests are camelCase in either `.ts` or `.tsx`.
- **`ignoreRestSiblings: false`.** `const { a: _a, ...rest } = obj` is
  reported, and an underscore prefix doesn't exempt it. To drop
  routing-only fields, copy the object and `delete` them (they must be
  optional in the type). To remove a key from a record, do the same.
- **`--report-unused-disable-directives --max-warnings 0`.** Every
  `react-hooks/exhaustive-deps` warning fails the lint.

## Fixing `exhaustive-deps` without changing behaviour

Adding a reported dependency mechanically can turn a mount-only effect into
a resubscribe loop, or re-seed an open form. Patterns that keep firing
behaviour identical:

- **Helper that closes over imports only:** move it to module scope. It
  needs no dependency entry.
- **Callback that reads only refs:** wrap it in `useCallback(..., [])` and
  list it. Its identity never changes.
- **`x ?? []` or `.filter(...)` reported as "changes on every render":**
  wrap it in `useMemo` keyed on the source. A literal fallback can become a
  module constant instead. The value is the same; only the identity
  stabilises.
- **Ref cleanup warning ("ref value will likely have changed"):** when the
  ref holds a `Set` or `Map` that is never reassigned, capture
  `ref.current` at the start of the effect and use that in cleanup.
- **A dependency left out on purpose because it must not retrigger the
  effect:** read it through a ref that updates on every render
  (`ref.current = value` during render). For example, `McpServerDialog`
  re-seeds only on open, codec or initial, and a late `profiles` list must
  not wipe an open form.
- **`useMemo(() => ref.current, [unrelatedState])` (memo used as an
  invalidation trigger):** hold the value in state, set in the same place
  the ref is written. Then drop any reset that only re-ran the memo, since
  it returned the same array the state already holds.
- **Stable values** (`useQueryClient()`, a caller's `useRef` object,
  `useVirtualizer`'s instance, a context-provided singleton): adding them is
  free. Removing unused ones is equally safe.
- **`t` from `useTranslation`** changes only when the language changes, so
  adding it fixes a stale-language closure rather than adding churn.

## Verifying a lint change

- Hash `eslint --print-config <file>` before and after for sample files
  from each package to prove a refactor of the config is neutral.
- Negative proof: add an unused import to one file and check that
  `npm run lint` exits 1, then revert.
- Vitest mocks by resolved module id, so `vi.mock('@/features/x/model/y')`
  still applies when the component under test imports `../model/y`
  relatively.

## Contributed by

- vk/848f-lint-packages-we
