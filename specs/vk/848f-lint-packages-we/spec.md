# Feature Specification: Lint web-core and remote-web

**Feature dir**: `specs/vk/848f-lint-packages-we/`
**Task**: `vk/848f-lint-packages-we`
**Status**: Clarified

## Summary
`pnpm run lint` and CI's `frontend-checks` job run ESLint only over
`packages/local-web` and `packages/ui`. `packages/web-core` holds almost all
of the frontend code that local-web and remote-web ship, and it has no ESLint
config and no `lint` script. `packages/remote-web` has the same gap. Whatever
lands in those two packages, the repo-wide lint stays green. This feature
brings both packages under the same lint rules as local-web, in both the
local command and CI, and fixes what the rules find today, without changing
runtime behaviour.

## User Stories
- As a contributor, I want `pnpm run lint` to flag an unused import or a
  broken hook dependency list in web-core, so I find it before review.
- As a reviewer, I want CI to fail when a web-core or remote-web change breaks
  a lint rule, so I don't have to catch it by eye.
- As a maintainer, I want one definition of the frontend lint rules, so that
  local-web, web-core and remote-web can't drift apart.

## Functional Requirements
- FR-1: `pnpm run lint` runs ESLint over `packages/web-core` and
  `packages/remote-web`, in addition to the packages it covers today.
- FR-2: Each package can be linted on its own from its directory
  (`npm run lint`), with the same flags local-web uses: `--ext ts,tsx`,
  `--report-unused-disable-directives` and `--max-warnings 0`.
- FR-3: CI's `frontend-checks` job runs both new lints, and the job fails
  when either reports any error or warning.
- FR-4: The rules applied to web-core and remote-web are local-web's rules,
  taken from one shared definition rather than copied. That means restricted
  imports and syntax, layer boundaries, unused imports and variables, React
  hooks, the opt-in i18n check, the ban on ESLint directive comments, and file
  naming. Rules that only describe local-web's legacy layout may stay in the
  shared definition as long as they match nothing in the other packages.
- FR-5: Every linted file, including `*.test.ts(x)` and the package's
  `*.config.ts`, parses against a tsconfig that includes it. No parse error
  comes from tsconfig scoping.
- FR-6: Every existing finding in both packages is fixed. No rule is disabled
  for a whole package. A genuine exception names the file it covers and says
  why.
- FR-7: Each `react-hooks/exhaustive-deps` fix keeps the hook's current
  firing behaviour: when it runs, how often, and what it subscribes to.
  Missing dependencies are not added mechanically.
- FR-8: Local-web's lint result does not change. Its resolved config is
  unchanged apart from the shared naming-rule map (see Clarifications), and
  that map matches no local-web file differently.
- FR-9: After this lands, the wiki page that documents the old workaround
  (`wiki/workspace-sidebar-filtering.md`, "Verification notes") points at the
  real lint command instead.

## Out of Scope
- Moving to ESLint 9 flat config, or upgrading any lint plugin.
- New rules beyond what local-web already enforces.
- Linting `packages/public` (static assets) or `npx-cli`.
- Refactoring beyond what a finding needs. Where fixing a finding means moving
  a module, the move changes import paths only.

## Acceptance Criteria
- [ ] `pnpm run lint` invokes `web-core:lint` and `remote-web:lint`, and both
      exit 0 with zero warnings.
- [ ] `.github/workflows/test.yml` `frontend-checks` runs
      `cd packages/web-core && npm run lint` and
      `cd packages/remote-web && npm run lint`.
- [ ] Adding an unused import to a web-core file and to a remote-web file
      makes the matching `npm run lint` exit non-zero. Removing it restores
      exit 0.
- [ ] No "TSConfig does not include this file" parsing errors in either
      package, including for test files.
- [ ] No package-wide rule disable. Any per-file exception is listed in the
      shared config next to its reason.
- [ ] `pnpm --filter @vibe/web-core test`, `pnpm --filter @vibe/remote-web
      test`, `pnpm run check` and `pnpm run format` pass.
- [ ] `npx eslint --print-config` for local-web files is byte-identical
      before and after the extraction (before the naming-map edit), and
      local-web lint stays green after it.
- [ ] The wiki "Verification notes" no longer documents the
      `--no-eslintrc` workaround.

## Open Questions
- Resolved in clarify: see "Clarifications" below.

## Clarifications
- Q: The issue counted 43 findings in web-core with local-web's config
  pointed at it. With web-core's own config, the path-scoped rules apply too.
  Should those extra findings be fixed or scoped out?
  A: Fix them. Measured on main @ 292aba26, web-core's own config reports 61
  problems: 24 `exhaustive-deps`, 14 unused variables (7 each from two
  rules), 10 file naming, 8 layer-boundary imports, 3 directive comments,
  1 `no-empty`, 1 unknown rule (`jsx-a11y/alt-text`, which is a directive
  naming a plugin that isn't installed). The issue's single
  `no-restricted-syntax` finding is gone on current main. The layer and
  naming findings are real: shared code importing features and pages, and
  hook files named in camelCase while the rule demands PascalCase for every
  `.tsx`.
- Q: The issue allows "justified inline disables", but local-web's
  `eslint-comments/no-use` rule bans every directive comment. Which wins?
  A: The shared rule. Inline directives stay banned in every package. Any
  justified exception goes in the shared config, scoped to the named file,
  with its reason.
- Q: Should hook files (`use*.tsx`) and their tests have to be PascalCase?
  A: No. The naming rule already makes `use*.ts` camelCase. It extends that to
  `use*.tsx`, and PascalCase applies to every other `.tsx`. This changes no
  local-web file, because local-web has no `use*.tsx`.
- Q: remote-web: in this task or a separate issue?
  A: In this task. It has 13 findings across 6 files.
- Q: Layer boundary violations (shared importing from features or pages):
  move the modules or file an exception?
  A: Move them, changing only import paths. `createModeSeedStore` depends
  only on shared types, so it moves to `shared/stores`. The create-mode chat
  box containers move into `features/create-mode/ui`. `SharedAppLayout`
  composes page containers, so it moves to the pages layer. Same-feature
  imports through the `@/features/<own>` alias become relative imports,
  which is how the other 60 intra-feature imports are already written.
- Q: `settingsRegistry.tsx` is a camelCase `.tsx` that is not a hook. Rename
  it or add an exception?
  A: Rename it to `SettingsRegistry.tsx`. It has two importers, and a rename
  leaves no exception behind.
- Q: Does the `*.config.ts` in each package get linted?
  A: Yes. `eslint .` reaches `vite.config.ts` and `vitest.config.ts`. The
  shared config already turns off type-aware parsing for config files, and
  each package's lint tsconfig also includes them, so they parse either way.

No open questions remain.
