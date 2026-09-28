# SPEC — GitHub organization tokens (per-owner fine-grained PATs)

Task: `vk/0f52-manage-gh-token`

## Problem

Workspace processes (coding agents, scripts, dev servers, terminals) get
GitHub credentials from ambient environment variables. Today that means an
organization Env Var such as `GITHUB_TOKEN`/`GH_TOKEN`, and one variable can
only hold one PAT. A fine-grained PAT is scoped to a single resource owner, so
a workspace that mixes repositories from several GitHub owners cannot work with
one token. This task's own workspace is an example: `davidvasandani/homelab`,
`davidvasandani/vibe-kanban`, and `sweetgreen/platform-ops`. Operators work
around it with extra variables (`GITHUB_TOKEN_ALDERBRIDGE`, …) that the GitHub
CLI and Git never read, so `gh pr create` and `git push` use whichever token
happens to be in `GH_TOKEN`/`GITHUB_TOKEN`.

The homelab deployment already has a Nix-level `gh` router
(`services.vibe-kanban-rebuild.githubAuth.orgTokenRefs`), but no host has it
configured. It needs a Nix change and a service restart for every token, has no
UI, and covers only `gh`, not `git push`.

## Goal

Settings → Repositories gets a **GitHub organization tokens** section. Each row
maps a GitHub owner (organization or user) to a fine-grained PAT, entered either
as a literal token or as a 1Password `op://` reference. Every workspace session
then uses the matching owner's PAT:

- for `gh` commands, by the repository each command targets;
- for Git HTTPS operations against `https://github.com/<owner>/…`.

Different owners never share a PAT, and nothing collides in `GH_TOKEN`.

## Behavior

### Configuration (machine-scoped)

- Stored on the Vibe Kanban host that owns the settings (the coordinator in a
  cluster), like the other Machine Settings. Settings for another machine go
  through the machine selector, as on the rest of the page.
- Owner: a GitHub login of 1–39 characters (`[A-Za-z0-9-]`, no leading or
  trailing hyphen). Owners are unique without regard to case.
- Value: a literal PAT or a 1Password reference. Quotes that 1Password adds to
  a copied reference are stripped, using the same `normalize_secret_reference`
  rule as organization Env Vars.
- Values are encrypted at rest with a host-bound key. Literal values are
  write-only: the list API never returns them, and the UI shows `••••••••`.
  1Password references are pointers, not secrets, so they are returned and
  shown.
- Actions: add, replace the value, delete.

### Delivery to workspace processes

- At every workspace process launch (initial, follow-up, review, setup/cleanup
  scripts, dev servers, background helpers, workspace terminals, local or on a
  cluster worker), the coordinator resolves each configured value. It uses the
  existing Env Var resolver (`services::environment_secrets`), so the same
  `OP_SERVICE_ACCOUNT_TOKEN` source and the same errors apply. If a configured
  reference cannot be resolved, the launch fails with a secret-safe error, the
  same as an Env Var reference.
- Resolved tokens are passed as internal `VK_GITHUB_PAT_<OWNER>` variables,
  plus a `VK_GITHUB_PAT_OWNERS` manifest. The `VK_` prefix is reserved, so
  organization Env Vars cannot spoof them. They reach a cluster worker in the
  same dispatched environment that already carries organization Env Vars. No
  new transport is added.
- The host that spawns the process (coordinator or worker) prepares its own
  environment just before spawn:
  - it prepends a node-local, app-owned shim directory containing `gh` to
    `PATH`;
  - it appends owner-scoped Git config through `GIT_CONFIG_COUNT`/`KEY_n`/
    `VALUE_n`. For each owner this is
    `credential.https://github.com/<owner>.helper` = reset, then an inline
    helper that prints that owner's token variable. It continues after any
    `GIT_CONFIG_*` entries that already exist.
- With nothing configured, launches are unchanged: no shim on `PATH`, no extra
  variables.

### `gh` routing (shim)

- The target repository comes from `-R/--repo/--repo=` if given. Otherwise it
  is the current directory's Git remote, chosen in this order:
  `remote.pushDefault` → branch `pushRemote` → branch `remote` → `origin` →
  the only remote. This is the same order as the homelab router.
- Accepted forms are `OWNER/REPO`, `https://github.com/…`,
  `ssh://git@github.com/…`, and `git@github.com:…`. The owner is matched
  without regard to case.
- Configured owner: sets `GH_TOKEN` to that owner's PAT for the real `gh` child.
  This overrides an ambient `GH_TOKEN`/`GITHUB_TOKEN`.
- Unconfigured owner, another host, or no repository context: runs the real
  `gh` with the caller's environment unchanged.
- Configured owner whose token is missing or empty: exits 78 with an error that
  names the owner and never prints a value.
- The real `gh` is the next one on `PATH` after the shim directory. If there is
  none, the shim exits 127 with an actionable message.

### Git routing

- HTTPS remotes under a configured owner get `username=x-access-token`,
  `password=<owner PAT>` from the owner-scoped helper. Other owners and hosts
  keep the machine's existing credential helpers.
- Git URL matching is case-sensitive. Contexts are emitted for the owner as
  entered and in lowercase.

## Non-goals

- VK server-side GitHub operations (PR creation and status from the UI) keep
  their current authentication.
- SSH remotes (keys, not PATs), GitHub Enterprise hosts, and more than one PAT
  per owner.
- Creating PATs or checking their permissions through GitHub.
- Changing the existing homelab Nix router. It stays as it is, is unconfigured
  fleet-wide, and chains after the app shim if it is ever enabled.
- Exporting `GH_TOKEN`/`GITHUB_TOKEN` session-wide. Only `gh` and Git are
  routed.

## Acceptance criteria

1. Settings → Repositories lists, adds, replaces, and deletes GitHub owner
   tokens. Literal values are never returned by the API and are shown masked.
   References are shown as entered, without the copied quotes.
2. A duplicate owner that differs only in case is rejected with an actionable
   message, and so is an invalid owner.
3. In a workspace with repos from owners A and B, `gh` run inside each repo
   gets that owner's PAT, and `gh --repo B/x` run from repo A gets B's PAT.
4. `git credential fill` for `https://github.com/A/r.git` returns A's PAT. For
   an unconfigured owner, the existing helper result is unchanged.
5. With no configuration, the spawn environment is byte-for-byte unchanged.
6. A configured `op://` reference that fails to resolve blocks the launch with
   the existing secret-safe resolver error. Tokens never appear in logs, errors,
   or API list responses.
7. Local execution, cluster-worker execution, and both terminal paths apply the
   same augmentation on the host that spawns.
8. Tests cover the shim (with a fake `gh`), the Git config, env augmentation,
   the store/API validation, and the frontend helpers. `pnpm run check`,
   `pnpm run lint`, `cargo test` for the touched crates, and
   `generate-types:check` pass.
