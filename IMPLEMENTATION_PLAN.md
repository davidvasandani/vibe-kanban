# Implementation plan: az and mgc-beta in every agent shell

Spec: `SPEC.md`. SpecKit artifacts:
`homelab/specs/vk/1d4e-fix-az-and-mgc-b/`. There are two pull requests, one
per repository. The app change is inert until the deployment exports the
contract, and the deployment change is harmless against an older app, so they
can merge in either order.

## Contract between app and deployment

| Name | Set by | Consumed by | Meaning |
| --- | --- | --- | --- |
| `VK_AGENT_PATH` | VK app, at every agent env boundary (identical to the `PATH` it computed) | host login-shell init | the PATH a login shell must restore |
| `VK_ENTRA_DBUS_RUN_SESSION`, `VK_ENTRA_KEYRING_DAEMON`, `VK_ENTRA_LIBSECRET_LIB` | deployment, on every agent-launching unit | mgc-beta wrapper | non-secret Graph CLI toolchain paths |

`VK_` names are already reserved against organisation env vars
(`RESERVED_ENV_PREFIXES`), so a workspace setting cannot spoof `VK_AGENT_PATH`.

## Vibe Kanban (this repository)

1. **`crates/utils/src/shell.rs`**: add `AGENT_PATH_ENV = "VK_AGENT_PATH"` and
   `agent_path(inherited) -> OsString`, which appends the managed bin when it
   exists and otherwise returns the inherited PATH unchanged. Unit-test the
   ordering (inherited first, managed bin last, no duplicates).
2. **Agent env boundaries** set both `PATH` and `VK_AGENT_PATH` from
   `agent_path`:
   - `crates/local-deployment/src/container.rs` (local executions, ~L4657);
   - `crates/worker/src/execution.rs::run_job` (worker executions);
   - `crates/worker/src/terminal.rs` (worker PTY terminals).
   Add a worker test asserting that the spawned environment carries
   `VK_AGENT_PATH == PATH`.
3. **mgc-beta wrapper** (`graph_cli_wrapper_script` in
   `crates/services/src/services/cli_tools.rs`):
   - replace the `${VAR:?}` guards with a `need` function. It checks that each
     variable is set and that its target exists (`-x` for executables, `-d`
     for the lib dir). Otherwise it prints
     `vibe-kanban: runtime dependency unavailable: <VAR> …` to stderr and
     exits `69` (`EX_UNAVAILABLE`, the new const
     `RUNTIME_DEPENDENCY_UNAVAILABLE_EXIT`). The message names only the
     variable, never a path value.
   - add `VK_CLI_TOOL_CHECK=1`: validate the dependencies and exit 0 without
     launching dbus, the keyring or the tool (used by the worker check).
   - keep `BIN`, `PW` and `XDG_DATA_HOME` unchanged, so credential isolation
     and existing caches are preserved.
   - `write_runtime_wrapper` writes atomically (unique tmp file, then rename)
     and only when the content differs. `status()` calls it, so an existing
     install picks up the new wrapper without a reinstall.
   - behavioral tests run the generated script with temporary fake
     `dbus-run-session`/keyring executables: missing variable, stale path,
     check mode, and successful exec that forwards args and exit status.
4. **Probe classification** (`probe_auth`): exit 69 with the vibe-kanban stderr
   prefix gives `Unknown` + "Runtime dependency unavailable: …" instead of
   `Unauthenticated`. The classification is a pure function
   `classify_probe_output(status, stderr)` with unit tests.
5. **Agent check** (new `CliToolAgentCheck` on `CliToolStatus`, exported to TS):
   - build the agent environment exactly as the boundaries do (process env,
     plus `PATH`/`VK_AGENT_PATH` from `agent_path`);
   - resolve `binary_name` in a non-login shell (`sh -c 'command -v …'`) and in
     a login shell (`$SHELL -lc`, falling back to `bash`/`sh`);
   - run the resolved command with `version_args` (and `VK_CLI_TOOL_CHECK=1`
     for wrapped tools first) under a timeout;
   - classify as `available` / `missing` / `login_shell_missing` /
     `runtime_unavailable` / `failed`, with paths, version and a sanitized
     message. The pure classification function gets unit tests.
6. **Worker startup check** (`crates/worker`): once at start, for each entry
   in the managed bin dir, resolve it in login and non-login shells with the
   worker's agent env, and run wrappers in check mode. Log `warn!` per
   unusable tool (name, classification, sanitized reason) and `info!` summary
   otherwise. The shared logic lives in `crates/utils/src/agent_tools.rs`,
   so the server and the worker use the same code.
7. **UI** (`CliToolsSettingsSection.tsx` + 7 locale files): replace the "agent
   not verified" copy with the agent-check line. Show the resolved path when
   `available`, or the classification message (warning style) otherwise.
8. `pnpm run generate-types`, then `pnpm run format`, `pnpm run check`,
   `pnpm run lint`, and `cargo test -p utils -p services -p worker -p local-deployment`.
9. **Docs**: `docs/settings/cli-tools.mdx` gets an "Agent environment" section
   covering the login-shell contract, the runtime variables, how existing
   sessions pick up changes (the env is fixed per agent process, so the next
   turn or a restarted session gets it, while a running shell does not), and
   the troubleshooting classifications.

## Homelab (deployment)

1. **`modules/lib/vibe-kanban-entra.nix`**: split out `graphCliRuntimeEnv`
   (the three non-secret toolchain variables) and export it as `runtimeEnv`,
   **not** gated on `entra.enable`. `VK_ENTRA_CHROMIUM` stays sign-in-only.
2. **`modules/vibe-kanban-rebuild.nix`**:
   - worker unit env: `// entra.runtimeEnv`;
   - `systemd.services.vibe-kanban-dev.environment`: add `entra.runtimeEnv`
     (a no-op on think2, which already gets it from `entra.env`);
   - `environment.extraInit` (when the module is enabled) adds a guarded
     login-shell hook: `if [ -n "${VK_AGENT_PATH-}" ]; then PATH="$VK_AGENT_PATH:$PATH"; fi`.
     `extraInit` is part of `set-environment`, so it runs exactly when NixOS
     has just replaced PATH, and not otherwise.
3. **`modules/vibe-kanban.nix`**: the same runtime env for the base/dev
   services, and the same hook when enabled. Keep one definition by sharing it
   from the entra lib.
4. **Checks**: a `ci/check-vibe-kanban-agent-shell.sh` (or the existing
   eval-test location) that (a) evaluates think1's worker unit env and asserts
   the three runtime variables are present with executable/dir targets, without
   `entra.enable`; (b) evaluates `environment.extraInit` and asserts the hook;
   (c) runs the hook under `env -i bash --noprofile` with a fake
   `VK_AGENT_PATH` and asserts the precedence. There is also a negative
   control: without `VK_AGENT_PATH`, PATH is unchanged.

## Verification on the running cluster

This is limited by what is reachable from a worker session. Reproduce the
failure on think4 (done: `codex exec` + `bash -lc` gives MISSING). After the
app build, show that `env -u __NIXOS_SET_ENVIRONMENT_DONE VK_AGENT_PATH=$PATH
bash -lc 'command -v az mgc-beta'` resolves with the hook sourced from the
evaluated `extraInit`. Show that the wrapper check mode yields exit 69 with the
variables unset and exit 0 with them set. The full live acceptance (fresh
session, then restart, then service restart on a worker) needs the deployment
to roll out, which happens through the normal homelab rebuild after merge. It
is recorded as a post-deploy check, not claimed here.

## Rollback

Revert either PR. Without the hook, `VK_AGENT_PATH` is inert. Without the app
change, the hook does nothing because the variable is unset. The wrapper
rewrite keeps the same credential paths, so rolling back regenerates the old
script over the same state.
