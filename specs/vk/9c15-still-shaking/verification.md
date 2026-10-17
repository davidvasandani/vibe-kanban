# Verification: Stable Earlier-History Loading Geometry

## Pre-Fix Regression Evidence

With `EarlierHistoryControl.test.tsx` present before the component/fix, the
focused run failed to resolve `./EarlierHistoryControl`; all 53 previously
existing test files and 387 tests passed. This establishes the new contract as a
fail-before-production-change gate.

## Passing Verification

- `pnpm --filter @vibe/web-core exec vitest run` for the earlier-history
  component, history paging, and plan reveal: 3 files, 18 tests passed.
- `pnpm --filter @vibe/web-core check`: passed.
- `pnpm run local-web:check`: passed.
- `pnpm run remote-web:check`: passed.
- `pnpm run ui:check`: passed.
- `pnpm run local-web:lint`: passed with zero warnings.
- `pnpm run format`: passed; touched files were already formatted.
- `git diff --check`: passed.

## Acceptance Evidence

- Idle and loading presentations occupy one grid cell and both always
  participate in intrinsic sizing, eliminating their prior normal-flow height
  delta even when text wraps or is enlarged.
- Loading progress and retry feedback remain visible and accessible.
- Pagination, semantic anchor correction, virtualization, bottom lock, and PR
  #268's plan reveal lifecycle are unchanged.
- No backend, deployment, homelab, generated type, or other-service files were
  changed.

Independent review and PR delivery are recorded by subsequent pipeline stages.

## Independent Review

Four Codex CLI passes produced three rounds of confirmed findings, all fixed and
reverified. The final pass reported no actionable correctness regressions. See
`review.md`.
