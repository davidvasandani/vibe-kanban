# SPEC: Lint web-core and remote-web like local-web

Task: `vk/848f-lint-packages-we`. Feature spec, plan and tasks:
`specs/vk/848f-lint-packages-we/`.

## Problem

`pnpm run lint` and CI's `frontend-checks` job run ESLint only over
`packages/local-web` and `packages/ui`. `packages/web-core` holds nearly all
of the frontend code that both apps ship, and it has no ESLint config and no
`lint` script. `packages/remote-web` is in the same position. Lint stayed
green whatever landed in those packages.

## Measured state (main @ 292aba26)

- Pointing local-web's config at web-core gives 174 problems. Most are
  parse errors, because local-web's `parserOptions.project` doesn't include
  web-core's files, and web-core's own `tsconfig.json` excludes its tests.
- With web-core's own config, the path-scoped rules apply properly: 61
  problems. That's 24 `exhaustive-deps`, 14 unused variables, 10 file names,
  8 layer-boundary imports, 3 directive comments (banned by
  `eslint-comments/no-use`), 1 `no-empty`, and 1 directive naming a rule from
  the uninstalled `jsx-a11y` plugin. Removing one directive uncovered a
  further `exhaustive-deps` site in `McpServerDialog`.
- Remote-web: 13 problems. That's 5 `exhaustive-deps`, 6 unused variables,
  1 non-exhaustive switch, and 1 barrel re-export.

## Design

### Shared rule set
- `eslint.frontend.cjs` (repo root) exports
  `createFrontendConfig({ project, ignorePatterns })`. Its body is local-web's
  former `.eslintrc.cjs`, moved as is. Each package's `.eslintrc.cjs` is a
  call that supplies its own tsconfig and ignores.
- The config objects are inlined with `require`, so ESLint resolves plugins
  from the consuming package. Remote-web therefore declares the same ESLint
  devDependencies (same versions) as web-core and local-web.
- Before the naming-rule edit, the extraction is verified by hashing
  `eslint --print-config` output: identical for local-web files.

### Type-aware parsing that covers tests
`packages/{web-core,remote-web}/tsconfig.eslint.json` extend the package
tsconfig, include `src` and `*.config.ts`, and clear the test `exclude`.
`tsc --noEmit` is unaffected.

### Wiring
- Package `lint` scripts use local-web's flags (`--ext ts,tsx
  --report-unused-disable-directives --max-warnings 0`).
- The root has `web-core:lint` and `remote-web:lint`, and both are part of
  `pnpm run lint`.
- CI `frontend-checks` runs both. `eslint.frontend.cjs` is added to the
  workflow's `frontend` path filter, so a change to only the shared config
  still triggers the job.

### Rule adjustment
In `check-file/filename-naming-convention`, PascalCase applies to
`src/**/!(use*).tsx`, and `src/**/use*.{ts,tsx}` is camelCase. Before, hooks
that render a provider (`useAppRuntime.tsx`) and hook tests
(`useWorkspaces.test.tsx`) had to be PascalCase. Local-web has no
`use*.tsx`, so its result is unchanged.

### Findings fixed in code
- **Layer boundaries.** Modules move and only import paths change.
  `createModeSeedStore` → `shared/stores`. `CreateChatBoxContainer` and
  `CreateModeRepoPickerBar` → `features/create-mode/ui`. `SharedAppLayout`
  (composes page containers) → `pages/root`. Same-feature aliased imports
  become relative.
- **Naming.** `settingsRegistry.tsx` → `SettingsRegistry.tsx`.
- **Unused variables.** Routing-only fields are dropped with a shallow copy
  plus `delete` instead of a rest-destructure into `_x` names, and unused
  parameters are removed.
- **Directive comments.** Removed. `rehypePlugins` is typed with
  react-markdown's `Options`.
- **Remote-web.** Explicit no-op cases for the outbound-only `http_request`
  and `ws_open` messages. The webrtc barrel is deleted.
- **`exhaustive-deps`.** Each site gets its own judgement (plan §7). Pure
  helpers move to module scope, ref-only callbacks become stable
  `useCallback([])`, `?? []` fallbacks are memoized, and unused deps are
  dropped. Stable values (`queryClient`, the `scrollContainerRef` object,
  `appNavigation`, `t`) are listed as deps. Two cases needed more than that:
  - `ConversationListContainer` read a ref through a `useMemo` keyed on
    unrelated state. It now holds the rows in state, set in the same flush.
  - `McpServerDialog` must not re-seed an open form when `profiles` arrives
    late (constitution X). It reads `profiles` through a ref that updates on
    every render.

## Non-goals
Migrating to ESLint 9 or upgrading plugins, adding rules local-web doesn't
have, and linting `packages/public` or `npx-cli`.

## Acceptance
- `pnpm run lint` exits 0, including `web-core:lint` and `remote-web:lint`.
- Each new lint exits 1 when an unused import is added to one of its files.
- No tsconfig-scoping parse errors, test files included.
- No package-wide disables and no inline directives.
- `pnpm run check`, `pnpm run format`, and the web-core (664 tests) and
  remote-web (87 tests) vitest suites pass.
- The wiki no longer documents the `--no-eslintrc` workaround.
