# Implementation Plan: Say which 1Password lookup failed, and why

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

- Rust 2024, tokio, thiserror. Repository: `vibe-kanban` (the Vibe Kanban
  service only; the homelab deployment is unchanged).
- The resolver is `crates/services/src/services/environment_secrets.rs`. It
  spawns `op read --no-newline -- <ref>` with `OP_SERVICE_ACCOUNT_TOKEN` in
  the environment and all other `OP_*` cleared. stdout is bounded to 64 KiB
  and stderr currently goes to `/dev/null`.
- Callers:
  - `crates/local-deployment/src/container.rs` `resolve_org_env_vars`
    (org Env Vars → `ContainerError::EnvironmentSecret`, then shown as
    "Failed to start execution: {error}" by `services/container.rs`).
  - `crates/services/src/services/github_owner_tokens.rs`
    `launch_environment` (wrapped as "GitHub token for {owner}: …").
  - `crates/services/src/services/github_credentials.rs` `resolve_reference`
    (owner made *unavailable* with the message).
  - `crates/server/src/error.rs` maps `EnvironmentSecret` to 400 with
    `to_string()`.

## Architecture & Approach

1. `LookupFailure` enum with a fixed `Display` remedy per cause, and a
   private `classify(&[u8])` that matches needles case-insensitively
   (order: rate limit → token rejected → not found → unknown).
2. `EnvironmentSecretError::ReadFailed { variable: Option<String>, cause }`,
   displayed as `1Password lookup[ for NAME] failed: <remedy>`.
3. `read_reference` pipes stderr, reads stdout (≤64 KiB + 1) and stderr
   (≤8 KiB) concurrently with `tokio::join!`, and drops both pipes before
   `wait()` so a chatty child cannot block. A spawn or IO error gives
   `Unknown`, a non-zero exit gives `classify(stderr)`, and the buffer is
   dropped.
4. `resolve_with_binary` attaches the map key to a `ReadFailed` and emits a
   `tracing::warn!(variable, cause)`.
5. `EnvironmentSecretError::without_variable()` is applied in both GitHub
   callers, where the key is the internal `VALUE`.
6. Docs: the failure paragraph in `docs/settings/organization-settings.mdx`
   lists the causes, and the quota with how to check it.

## Data Model

No persisted data changes. See `./data-model.md` for the error type.

## Contracts

See `./contracts/error-messages.md` for the user-visible message strings.

## Research Notes

See `./research.md`. It covers the observed `op` 2.34.1 stderr, the quota
evidence and the rejected alternatives. No new dependencies.

## Constitution Check

- **LV (added for this task)**: the failing input is named, the cause is
  classified in-process, and stderr is bounded and dropped. Honoured.
- **XLV**: GitHub paths keep owner attribution. Honoured.
- **II Test the contract**: classifier table tests, a fake-`op` integration
  test, a privacy test, and a GitHub-path naming test.
- **III Small steps**: one module plus two `map_err` call sites. Caching was
  deliberately left out.
- Homelab principle 13 (actionable, secret-safe errors) is consistent with
  this design.

## Risks & Dependencies

- `op` wording can change between versions. A miss falls back to `Unknown`,
  which keeps today's advice, so there is no regression.
- Needles such as "401" or "429" could match unrelated text. The order puts
  the least harmful misclassification last, and the text is never shown.
- Tests that set `OP_*` must not depend on the host's ambient token
  (wiki: host-env-sensitive-tests). The fake `op` tests pass explicit
  tokens.
