# Tasks: Three rollout loose ends

**Plan**: `./plan.md`

Tasks are dependency ordered. `[P]` tasks touch independent files and may run
together within their layer.

## Phase 1: Failing contracts and setup

- [x] T001 Run `pnpm install --frozen-lockfile` to establish the required
  worktree toolchain.
- [x] T002 [P] Capture current i18n failure and ordering diagnostics from
  `scripts/check-i18n.sh` (depends on T001).
- [x] T003 [P] Add the background-helper envelope contract test in
  `crates/server/src/routes/workspaces/execution.rs`.
- [x] T004 [P] Add Codex command/config regression tests in
  `crates/executors/src/executors/codex.rs`.

## Phase 2: Independent fixes

- [x] T005 [P] Add the six `metricsDiskAlerts` translations in
  `packages/web-core/src/i18n/locales/{es,fr,ja,ko,zh-Hans,zh-Hant}/common.json`
  and normalize the set comparison in `scripts/check-i18n.sh` (depends on T002).
- [x] T006 [P] Add actionable helper error messages and wire both rejection sites
  in `crates/server/src/routes/workspaces/execution.rs` (depends on T003).
- [x] T007 [P] Remove the dead Codex field/config emission and add verified
  `--strict-config` launch behavior in
  `crates/executors/src/executors/codex.rs` (depends on T004).
- [x] T008 Regenerate `shared/types.ts` and `shared/schemas/codex.json` with the
  repository generator (depends on T007).

## Phase 3: Focused verification

- [x] T009 [P] Run the full requested i18n reproduction and placeholder/key-set
  assertions (depends on T005).
- [x] T010 [P] Run focused server execution-route tests (depends on T006).
- [x] T011 [P] Run focused executor Codex tests and generated-contract checks
  (depends on T008).
- [x] T012 Record the bounded MCP-reachable `error_with_data` audit disposition
  in `research.md` (depends on T006).

## Phase 4: Broad verification and review

- [x] T013 Run `pnpm run format`, relevant frontend checks, backend checks, and
  lint/tests proportionate to the changed surfaces; record results in
  `verification.md` (depends on T009, T010, T011, T012).
- [x] T014 Run independent Codex CLI review, address confirmed findings, re-run
  affected checks, and write `review.md` until no significant findings remain
  (depends on T013).

## Phase 5: Knowledge and delivery

- [x] T015 Update reusable knowledge under `docs/knowledge-base/`, tag it
  `vk/94c0-three-loose-ends`, refresh `docs/knowledge-base/INDEX.md`, and commit
  the knowledge-base update (depends on T014).
- [ ] T016 Recheck the latest base, commit remaining scoped work, push
  `vk/94c0-three-loose-ends`, open a PR to `main`, wait for required CI, fix any
  failures, and merge (depends on T015).
