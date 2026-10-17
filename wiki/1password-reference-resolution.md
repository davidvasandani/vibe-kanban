# 1Password reference resolution and its failures

Organization Env Vars, and GitHub owner tokens, may hold `op://` references.
`services::environment_secrets` resolves them on the coordinator before each
launch by running `op read`. This page records how failures are reported and
how to debug one.

## Where a reference is resolved, and under which token

- **Org Env Vars:** `LocalContainerService::resolve_org_env_vars` runs on
  *every* execution start, including follow-ups. The org's literal
  `OP_SERVICE_ACCOUNT_TOKEN` wins over the service's ambient token.
- **1Password Connect first, when the org opts in.** If the org sets
  `OP_CONNECT_HOST` and a literal `OP_CONNECT_TOKEN`, each `op read` tries
  Connect first (10 s limit) and falls back to the service account. This
  mirrors the homelab `op-read` wrapper.
  - Each child gets exactly one credential, and every ambient `OP_*` is
    stripped.
  - After a Connect timeout, a process-wide breaker skips that host for 60 s.
    It is not a per-call flag, because one launch resolves Env Vars and each
    owner token in separate calls (Codex review), and launches overlap.
  - An `InvalidValue` from Connect is not refetched.
- **Why Connect is org-only, never ambient.** The coordinator's
  `vibe-kanban-dev` unit exports `OP_CONNECT_HOST`, but it `unset`s
  `OP_CONNECT_TOKEN` before exec. If the host's Connect were used
  automatically, any org admin could resolve anything Connect can see in
  `Homelab`, even items their own token cannot read. The user chose org
  opt-in (option A) over host-wide Connect.
- **GitHub owner tokens at launch** (`github_owner_tokens::launch_environment`)
  copy the org's `CREDENTIAL_VARIABLES` (service-account token and Connect
  pair) into each single-reference request. The server-side git and `gh` path
  (`github_credentials`) uses the server's token and caches results for 60 s.
- **Project vs org.** The org that owns a project is not obvious from its
  name. The project "Platform Ops" belongs to the **Sweetgreen** org, not the
  "Platform Ops" org. Check the project's organization before reading Env
  Vars.

## Failure messages are classified, never echoed

`op`'s stderr echoes the reference, vault, item and field names, so it is
never surfaced. Up to 8 KiB of it is read and mapped to a `LookupFailure`
(`RateLimited`, `Unauthorized`, `NotFound`, `Unknown`), and the message names
the env var: `1Password lookup for GH_TOKEN failed: <remedy>`. The GitHub
paths call `without_variable()` because their map key is the placeholder
`VALUE`. The owner already names the input.

Classifier gotchas (from Codex review):

- **Echoed names must not decide the cause.** An item named `token429` was
  read as a rate limit. Strip quoted segments first, and leave word-internal
  apostrophes (`isn't`, `David's`) alone so they do not open a quote.
- **Check `op`'s structural not-found phrasing first** ("isn't an item",
  "isn't a vault", "does not have a field"). It only appears after an
  authorized request. Require statuses in `op`'s `(NNN)` form, not bare
  digits.

Pipe-handling gotcha: reading stdout and stderr with `tokio::join!` on
`Take` adaptors that live past their bound deadlocks. The child blocks
writing the full pipe that nobody reads, and the other read never sees EOF.
Move each pipe *into* its own future, so it closes at its bound and the
child gets a broken pipe. Tests cover endless stdout (`yes`) and 20k lines
of stderr.

## Debugging recipe (no secret ever printed)

1. Find the failing session's timestamps:
   `search_session_messages(query="1Password")`.
2. List the org's references through the admin listing
   `GET /v1/organizations/{org}/env-vars`. It returns `reference` for `op://`
   values and never returns literals.
3. Test the identity and access the launch would use, with ambient `OP_*`
   stripped the same way the server strips them:
   `env -u OP_CONNECT_HOST -u OP_CONNECT_TOKEN op whoami`, then
   `op vault list`, then `op read <ref> >/dev/null; echo $?`.
4. Check quota: `op service-account ratelimit`. The homelab service account
   (`vasandani.1password.com`, vault `Homelab`) has an **account-wide limit of
   1,000 read/write requests per 24 hours**, shared by host Nix-auth refresh,
   the op-read fallback off Connect, owner-token resolution and every
   execution start. A failure that later clears with no configuration change
   most likely means this quota was used up (incident 2026-10-15, SWE-278).
   The fix is to configure the org's Connect pair. The homelab Connect server
   is `http://172.16.100.231:8081` on think4, and its client token lives at
   `/var/lib/onepassword-connect-client/token` on think2.

## Contributed by

- vk/76ab-debug-1password
