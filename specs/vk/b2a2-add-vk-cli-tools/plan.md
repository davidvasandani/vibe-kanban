# Implementation Plan: CLI Tools in Workspace Sessions

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

- Rust 2024 workspace.
- Managed tool directory authority:
  `crates/utils/src/assets.rs::cli_tools_dir`.
- Platform-aware, ordered path merge:
  `crates/utils/src/shell.rs::merge_paths`.
- Local managed execution:
  `crates/local-deployment/src/container.rs`.
- Local interactive terminal spawning:
  `crates/local-deployment/src/pty.rs`.
- Cluster execution and terminal spawning:
  `crates/worker/src/execution.rs` and `crates/worker/src/terminal.rs`.
- Coordinator dispatch payloads carry workspace/org environment values through
  `cluster-protocol`, but node-local runtime paths must be derived on the worker.
- No schema, database, frontend, generated TypeScript, or external dependency
  change is required.

## Architecture & Approach

### 1. Centralize the PATH contract in `utils`

Add a small helper near the existing path utilities that accepts an optional
current PATH and a managed bin path, returning the host-first de-duplicated PATH
only when the managed bin directory exists. Add a canonical convenience entry
point that derives `assets::cli_tools_dir().join("bin")` on the current process
host.

This keeps existence, ordering, and deduplication rules identical across local
and worker crates without making the lean worker depend on `services`.

### 2. Reuse the helper for local managed execution

Replace the inline merge in
`crates/local-deployment/src/container.rs::start_execution_inner` with the
shared helper. Preserve its current precedence: any `ExecutionEnv` PATH first,
then the inherited server PATH, then app-managed tools.

### 3. Apply the contract at both terminal spawn boundaries

In `crates/server/src/routes/terminal.rs`, augment PATH only on the local
workspace-terminal branch after clustered dispatch has been ruled out. The
generic `PtyService` remains unchanged because it also serves machine-scoped
managed-login sessions with a deliberately minimal environment.

In `crates/worker/src/terminal.rs`, perform the same merge on the worker just
before spawning the PTY. Do not add the coordinator's managed path to
`TerminalCreateRequest`.

### 4. Apply the contract to clustered managed execution

In `crates/worker/src/execution.rs::run_job`, add the worker-local managed bin
directory to the `ExecutionEnv` before spawning either raw command or executor
actions. The coordinator continues sending scoped organization and VK context;
the worker owns node-local PATH augmentation.

Do not add runtime PATH to the request digest on the coordinator. Node-local
path availability is execution-host state, not user-defined dispatch identity.

### 5. Validate every boundary

- Unit-test the shared helper for host-first ordering, preservation,
  deduplication, empty/missing path, and platform path separators.
- Cover local terminal environment assembly through the shared helper tests and
  retain existing PTY tests proving terminal-owned environment precedence.
- Extend worker execution and terminal tests to prove the worker merges its own
  PATH rather than trusting a coordinator path.
- Keep tests hermetic: use temporary directories and synthetic executable names;
  do not install a real vendor CLI or read credentials.

## Data Model

No persistent data model changes.

## Contracts

No wire-contract change. Existing `ExecutionDispatch.environment` and
`TerminalCreateRequest.environment` remain scoped coordinator-provided values;
the execution host adds its local runtime path at spawn time.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: focused tests cover the observable PATH contract.
- Principles III and VI: reuse `cli_tools_dir` and `merge_paths`; no parallel
  catalog or protocol mechanism.
- Principle VIII: only the stable `cli-tools/bin` directory is exposed and host
  copies remain first.
- Principles XVIII and XX: workers derive node-local runtime paths; no
  coordinator-only absolute path crosses nodes.
- Principle XXI: one shared helper implements the existing resolution rule.
- Constraint XIV: verification uses locked repository commands and formatting.

No deviations or open constitution questions.

## Risks & Dependencies

- Login or interactive shell initialization may later rewrite PATH. The feature
  guarantees the spawn environment; tests should invoke direct commands when
  asserting exact ordering.
- `asset_dir()` creates its directory. The helper must avoid causing unrelated
  application-data creation solely to test whether `cli-tools/bin` exists; use
  the canonical location consistently with existing runtime behavior and keep
  missing-bin handling non-fatal.
- Managed CLI login PTYs share `PtyService`; keep workspace PATH augmentation at
  the terminal route so those machine-scoped sessions retain their existing
  noninteractive allowlist behavior.
