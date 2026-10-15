# Tasks: Say which 1Password lookup failed, and why

Paths are relative to the `vibe-kanban` repository.

## Layer 1: core (one file, sequential)

- [x] T001 Add the `LookupFailure` enum with remedy `Display`, plus
      `classify(&[u8])`. Change `ReadFailed` to `{ variable, cause }` and add
      `without_variable()`. File:
      `crates/services/src/services/environment_secrets.rs`
- [x] T002 Pipe stderr in `read_command`. In `read_reference`, read stdout and
      stderr concurrently and in bounded amounts, and classify on a non-zero
      exit. Spawn and IO errors give `Unknown`. File: same as T001
- [x] T003 In `resolve_with_binary`, attach the key name to a `ReadFailed`
      and log a `warn!(variable, cause)`. File: same as T001

## Layer 2: callers and docs (independent files, after T001)

- [x] T004 [P] Apply `without_variable` in `resolve_reference` and update the
      test constructors. File:
      `crates/services/src/services/github_credentials.rs`
- [x] T005 [P] Apply `without_variable` in `launch_environment` and update
      the test constructors. File:
      `crates/services/src/services/github_owner_tokens.rs`
- [x] T006 [P] Document the causes and the shared quota. File:
      `docs/settings/organization-settings.mdx`

## Layer 3: tests (after T001–T003)

- [x] T007 Classifier table test, a fake-`op` rate-limit test that names the
      variable, privacy assertions, and a `without_variable` test. File:
      `crates/services/src/services/environment_secrets.rs`

## Layer 4: verify

- [x] T008 Run `cargo test -p services environment_secrets`,
      `cargo test -p services github_`, `cargo clippy -p services -p server
      -p local-deployment`, and `pnpm run format`.
