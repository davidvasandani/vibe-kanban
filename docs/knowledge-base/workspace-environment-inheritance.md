# Workspace environment inheritance

Tags: `6d24-org-env-vars-are`, `5e29-vk-github-fine-g`, `vk/b0d4-env-vars-value-f`, `vk/a63c-don-t-obfuscate`, `vk/2eb6-keep-env-vars-un`, `vk/0f52-manage-gh-token`

## One workspace has multiple process boundaries

“Available in the workspace” is broader than the coding-agent execution path.
Vibe Kanban starts setup scripts, agents, and development servers through
`ContainerService`, but interactive terminals are created independently through
`PtyService`. Configuration inheritance must be audited at every child-process
boundary; fixing only the managed executor path can leave the terminal with a
different environment.

## Resolve once, inject explicitly

Keep tenant lookup and filtering behind one workspace-scoped resolver. The
resolver maps the local workspace through its task/project to the remote project,
performs authenticated organization access, applies a short timeout, filters
reserved names, and degrades to an empty map. Consumers receive the resolved map
explicitly and pass it only to the child process.

Do not write secret `.env` files, mutate the long-lived server environment, or
duplicate remote lookup logic in terminal routes. Those approaches respectively
persist secrets, leak scope across concurrent workspaces, or allow security and
failure behavior to drift.

## Precedence belongs to the execution boundary

Apply inherited organization values before process-owned values. Reserved
application keys (`VK_*`, `PATH`, `HOME`, loader variables, executor auth wiring)
are filtered by the resolver. The PTY then owns terminal semantics such as
`TERM`, `COLORTERM`, prompt configuration, and `VIBE_KANBAN_TERMINAL`, so it
applies those last.

Non-workspace PTYs need an explicit choice too. Managed CLI login sessions pass
an empty workspace map and retain their minimal allowlisted host environment.

## Validation pattern

- Spawn a harmless direct PTY command with a synthetic inherited value and
  assert the child can read it.
- Supply a conflicting terminal-owned key and assert the PTY value wins.
- Unit-test the reserved-name boundary with both rejected runtime keys and
  accepted credential-style keys.
- Never use real credentials or log/debug-format the resolved map.

## Route credentials at the resource decision boundary

A workspace-wide environment variable is insufficient when one workspace can
contain repositories from several GitHub owners. GitHub CLI normally selects
credentials by host, and every GitHub.com organization shares that host. Route
the credential at each `gh` invocation, when both an explicit `--repo` target
and current-directory Git context are available.

Keep the long-lived Vibe Kanban process and cluster dispatch payload secret-free:
deployment resolves owner PATs into node-local runtime files, while the service
PATH contains only a wrapper and a non-secret owner routing table. The wrapper
introduces `GH_TOKEN` only to the final `gh` child. Workers independently resolve
the same owner map; never copy PATs through coordinator actions or shared NFS.

Precedence must be deterministic. An explicit repository target wins over Git
inference; configured owner credentials win over an ambient token; unknown
owners preserve existing authentication. Once an owner is configured, a
missing/empty credential fails locally rather than silently falling through to
another identity. Argument parsing stops at `--`, remote parsing is strict to
GitHub.com, and the real CLI is invoked by absolute path to prevent recursion.

For deployment-level wrappers, putting the wrapper first in the coordinator and
worker systemd `path` covers agents, scripts, dev servers, and workspace PTYs
without duplicating spawn logic. A credential-preparation oneshot that owns a
`RuntimeDirectory` must remain active (`RemainAfterExit=true`), or systemd
removes the directory as soon as preparation finishes.

**Superseded in practice by the app-owned router (`vk/0f52-manage-gh-token`).**
Per-owner PATs are now configured in Settings → Repositories and routed by an
app-written shim plus `GIT_CONFIG_PARAMETERS`, prepared on each executing host.
The coordinator resolves the values and sends them through the authenticated
dispatch environment, the same path resolved org Env Vars already take. That
is a deliberate exception to the "no PATs in dispatch" rule above, because a
UI-managed machine setting has no per-worker copy. The Nix router still
exists, but no host configures it. See `wiki/github-owner-token-routing.md`.

## Resolve 1Password references at environment preparation

Organisation Env Vars remain encrypted strings. The project env-vars endpoint
returns the owning organisation’s strings, not a separate project override layer.
`services::environment_secrets` interprets only whole values starting with `op://`
after the existing workspace lookup and reserved-name filter. Literal and returned
values are never interpolated or recursively resolved.

`ContainerService::resolve_org_env_vars` is fallible: callers in local execution,
worker dispatch, and terminal creation must propagate reference failures before
launch. Existing unavailable-remote-settings behavior remains best-effort, but an
explicit fetched reference must not silently fall back to a literal or empty map.
The typed error maps to an actionable HTTP response, with no raw provider output.

Use `cli_tools::effective_binary_for(CliToolId::Op)` for host-first/app-installed
CLI discovery. Pass `op read --no-newline -- <reference>` directly, with the token
in the child environment. A configured literal `OP_SERVICE_ACCOUNT_TOKEN` wins
over service-process fallback; an invalid configured token never selects another
identity. Remove ambient `OP_*` selectors/Connect/debug settings from the reader
before installing that token. Do not pass the workspace map wholesale to `op`.

Read results only into memory, preserve whitespace/newlines, reject NUL/invalid
UTF-8 and fields over 64 KiB, discard stderr, bound the full phase to 30 seconds,
and kill the reader on cancellation. CLI lookup is lazy for literal-only maps.
Resolve afresh for each preparation; existing running processes keep old values.
The coordinator passes results through the existing authenticated worker environment
transport. Do not write resolved values back to settings or secret `.env` files.

Tests use a fake CLI with synthetic data to check argument/token handling, byte
preservation, provider failures, limits, bootstrap precedence and cancellation.
A response-level test protects actionable error propagation through `ApiError`.

## Keep references readable: one normalization rule

A 1Password reference is a pointer, not a secret, so it stays visible both as a
draft and after it is saved. Literal values stay masked, and the server never
returns them.

**One rule, used everywhere.** `api_types::normalize_secret_reference`, mirrored
by `secretReference.ts` in web-core, decides what counts as a reference:
1. Trim the value.
2. Remove one matching pair of `"…"`, `'…'`, `“…”` or `‘…’` quotes, then trim
   again.
3. If the result starts with the case-sensitive prefix `op://`, it is a
   reference.

Anything else is a literal and is used byte for byte. The rule is applied in
three places. The remote create and update routes store references bare.
`environment_secrets` detects and reads references with it (and still rejects
a reference-shaped `OP_SERVICE_ACCOUNT_TOKEN`). The UI uses it for input types
and submitted payloads. Do not reintroduce bare `starts_with("op://")` checks:
1Password's *Copy Secret Reference* wraps the value in double quotes, and the
exact-prefix check from `vk/a63c` masked those pastes and then injected
`"op://…"` into agents literally.

**Tolerate on read as well as normalizing on write.** Rows saved with quotes
before the fix are not migrated. They work because the resolver and the listing
apply the same rule, which is cheaper and safer than rewriting encrypted rows.

**Surface references, never literals.** `OrganizationEnvVar.reference` is set
only on the admin-only listing and on create/update responses. The listing
decrypts server-side and keeps only values that normalize to a reference, so
filtering happens on the server, not in CSS. `query_as!` builds its target
struct from the selected columns, so adding an API field would break the build.
Point the macro at a private row struct instead. That keeps the SQL text, and
therefore the `crates/remote/.sqlx` offline cache, unchanged.

**Normalize drafts carefully.** Trimming on every keystroke drops a space typed
in the middle of a path (`op://Vault/alderbridge nix …`). While the user types,
only remove a quote pair (`reference !== value.trim()`). Apply the full
normalization at submit. Edit starts from the saved reference and stays empty
for literals. Rendered-DOM tests cover a quoted paste, the payloads, saved
reference versus literal rows, and the prefilled edit, all with synthetic data.
