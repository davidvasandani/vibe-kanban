# Tasks: OAuth login for managed CLI tools

**Feature**: `specs/003-cli-tool-oauth-login/`
**Task**: `vk/5a2a-vk-cli-tool-logi`

## Layer 1 — Contracts and test seams

- [x] T001 Add typed auth state/strategy metadata and effective-binary helpers to
      `crates/services/src/services/cli_tools.rs`.
- [x] T002 [P] Add unit-test fixtures for auth probes and catalog support without
      invoking real vendor logins.
- [x] T003 [P] Define PTY command/exit types and tests in
      `crates/local-deployment/src/pty.rs` while preserving shell-session API.

## Layer 2 — Backend behavior

- [x] T004 Implement bounded auth probes and extend `CliToolStatus`; classify
      uncertain or unsupported cases conservatively. Depends on T001-T002.
- [x] T005 [P] Implement direct command PTY sessions, explicit child termination,
      exit reporting, and lifecycle cleanup. Depends on T003.
- [x] T006 Verify the pinned `mgc-beta` login/status interface; enable it only if
      the probe is stable and non-secret, otherwise leave it unsupported with an
      explanation. Depends on T001.
- [x] T007 Add per-tool login-session conflict guards and 15-minute lifecycle
      management. Depends on T004-T005.
- [x] T008 Add the signed `/api/cli-tools/{id}/login/ws` route with fixed catalog
      commands, input/resize/cancel, exit/final-status messages, and cleanup.
      Depends on T007.
- [x] T009 [P] Add backend tests for catalog support and conflict locking, plus
      direct PTY output/exit lifecycle coverage. Depends on T008.

## Layer 3 — Generated and frontend contracts

- [x] T010 Export new Rust types in `generate_types.rs` and regenerate
      `shared/types.ts`. Depends on T004.
- [x] T011 [P] Extend the machine client with a socket-opening operation and
      typed messages, explicitly passing local host or relay-host scope (an
      endpoint string alone is insufficient). Depends on T008-T010.
- [x] T012 [P] Reuse the existing xterm primitives and machine-scoped socket
      plumbing without changing workspace terminal behavior. Depends on T005.

## Layer 4 — Settings experience

- [x] T013 Add auth-state labels and Login/Re-authenticate action visibility to
      CLI tool rows. Depends on T010.
- [x] T014 Add the login dialog using reusable xterm, clickable URLs, Cancel,
      Retry, result/error presentation, and final status refresh. Depends on
      T011-T013.
- [x] T015 [P] Add settings translations following the existing locale-key
      pattern. Depends on T013.
- [x] T016 Add frontend tests for auth-state/action visibility and keep socket
      message handling typed and exhaustive. Depends on T014-T015.

## Layer 5 — Verification

- [x] T017 Run type generation check, focused Rust tests, and focused frontend
      tests; fix all failures.
- [x] T018 [P] Run `pnpm run format` and verify no unrelated formatting changes.
- [x] T019 Run the web-core type check and focused test suite; resolve
      regressions. (This package has no standalone ESLint configuration.)
- [x] T020 Review the completed diff against spec acceptance criteria and tick
      every landed task.

## Parallelization Notes

Within a layer, tasks marked `[P]` touch independent seams and may run together.
Layers remain dependency ordered: contracts → backend → generated/frontend
plumbing → UI → verification.
