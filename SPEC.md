# Installed az and mgc-beta resolve and run in every agent shell

Task: `vk/1d4e-fix-az-and-mgc-b`. Spans this repository (the Vibe Kanban
service) and its deployment in the homelab repository
(`modules/vibe-kanban-rebuild.nix`, `modules/lib/vibe-kanban-entra.nix`,
`modules/vibe-kanban.nix`). No other service changes.

## Problem

On a cluster worker (think1, Codex session in workspace `46ed26db…`),
`command -v az` and `command -v mgc-beta` returned nothing, and the mgc-beta
wrapper could not start because `VK_ENTRA_DBUS_RUN_SESSION`,
`VK_ENTRA_KEYRING_DAEMON` and `VK_ENTRA_LIBSECRET_LIB` were unset. At the same
time the CLI Tools settings page showed mgc-beta as installed (0.2.3). The agent
concluded the tool was absent.

## Root causes (reproduced 2026-09-24 on think4)

1. **A login shell drops the agent PATH.** Codex runs every command as
   `/run/current-system/sw/bin/bash -lc '<cmd>'`. The supervised worker/service
   environment does not carry `__NIXOS_SET_ENVIRONMENT_DONE`, so NixOS
   `/etc/profile` sources `set-environment`, which *replaces* `PATH` with the
   system profile. The unit-provided `az` (`systemd…path`) and the appended
   `managed-cli-tools/bin` both disappear. Reproduced with a real
   `codex exec`: `command -v az mgc-beta` → nothing. Non-login shells (Claude
   Code's Bash tool) are unaffected, which is why the failure looked
   intermittent.
2. **The Graph CLI toolchain is coupled to the Entra sign-in switch.** The
   deployment exports the three wrapper variables only when a host sets
   `entra.enable`, and only think2 (the coordinator) does. Every worker runs
   agents against the *shared* `managed-cli-tools` directory, whose mgc-beta
   wrapper needs those variables on every invocation.
3. **Status is computed in the wrong environment and misclassifies failures.**
   The CLI Tools status probe runs inside the process that serves the page (the
   coordinator, where the variables exist) and runs the tool directly, not
   through the shell an agent uses. A wrapper that aborts on a missing
   dependency exits non-zero and is reported as *Unauthenticated*.

## Required behavior

- **R1.** An installed supported CLI (host-provided such as `az`, or
  app-managed under `managed-cli-tools/bin` such as `mgc-beta`) resolves by
  command name in agent-spawned login **and** non-login shells, with the same
  precedence in both (host copies before app-managed copies). No manual PATH
  edits, Nix-store searches or per-task exports.
- **R2.** The mgc-beta wrapper gets its D-Bus, keyring and libsecret runtime
  from the supported service environment on every host that can run agents,
  independent of whether that host performs Entra sign-in. Its isolated
  credential storage (wrapper-owned `XDG_DATA_HOME`, keyring password file) and
  any existing cached authentication are left unchanged.
- **R3.** When a runtime dependency is missing or no longer exists (for
  example, a garbage-collected store path), the wrapper fails with a distinct
  exit status and a sanitized message naming the missing variable. It must
  never be confused with "not authenticated".
- **R4.** CLI Tools status adds an **agent check**, computed in the environment
  real agents receive on that host: the path resolved in a non-login shell, the
  path resolved in a login shell, and a classification of `available`,
  `missing`, `login_shell_missing`, `runtime_unavailable` or `failed`, with an
  actionable message. The authentication state stays a separate fact.
- **R5.** Cluster workers verify the same agent check for the tools they expose
  when they start, and log any tool that agents on that host cannot use (name,
  classification, sanitized reason).
- **R6.** Behavior survives a workspace restart or resume, a VK service
  restart, and a NixOS activation, because it is derived from declared unit
  configuration and not from session state.
- **R7.** Validation output shows command paths, versions and sanitized failure
  classifications only. It never shows tokens, keyring passwords or
  authentication-cache contents.

## Acceptance

- In a fresh agent session on a worker, `command -v az`, `az version`,
  `command -v mgc-beta` and `mgc-beta --version` succeed through both
  `bash -c` and `bash -lc`, without exports or absolute paths.
- The same checks hold after a workspace restart or resume and after a VK
  service restart on the worker.
- Removing a runtime variable makes the wrapper exit with the dedicated status,
  and CLI Tools reports `runtime_unavailable` with the variable name rather
  than *Unauthenticated*.
- Regression tests cover: agent environments carrying the preserved agent PATH;
  wrapper dependency classification using temporary fixtures (no fixed store
  hash); probe classification; and the deployment's login-shell hook and
  runtime variables for a worker host that has no `entra.enable`.

## Out of scope

Azure CLI's `AADSTS65002` for PIM scopes is an OAuth client pre-authorization
issue, not a PATH issue. No tenant consent, role changes or token copying.
Surfacing each worker's agent check in the coordinator's settings UI would need
a cluster-protocol extension. It is recorded as a follow-up. This change covers
workers through the startup check (R5).
