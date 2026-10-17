# GitHub owner token routing

A workspace can mix repositories from several GitHub owners. Fine-grained PATs
are scoped to one owner, so a single `GH_TOKEN` cannot serve such a workspace.
Settings → Repositories → **GitHub organization tokens** stores one PAT per
owner. The credential is then chosen per `gh` invocation and per Git URL,
never per session. This page records the non-obvious decisions and the
failure modes that review turned up.

## Storage: its own table, never `Config`

`GET /api/info` returns the whole `Config` to the browser, and `PUT
/api/config` writes the whole object back. A secret stored in `Config` would
therefore be exposed, and any unrelated settings save could overwrite it.
Machine-scoped secrets get their own SQLite table (`github_owner_tokens`)
instead:

- **Queries.** Use runtime `sqlx::query_as` calls, like `mcp_gateway.rs`, so
  the `.sqlx` offline cache stays untouched.
- **Uniqueness.** Case-insensitive owner uniqueness is `UNIQUE COLLATE
  NOCASE`, enforced in the database. The unique-violation error is mapped to a
  409.
- **Encryption.** Values are `McpGatewaySecretStore` envelopes under a
  **separate** host key file, bound by AAD to the row id. A value copied from
  one row to another does not decrypt.
- **Write-only literals.** Only a normalized `op://` reference is ever
  returned. The empty-table path never touches the key file.

## Environment names must survive agent secret filters

Codex's default shell environment policy drops every variable whose **name**
contains `KEY`, `SECRET` or `TOKEN`. Anything a routed child needs must avoid
those words:

- **Token variables.** Named `VK_GITHUB_PAT_<HEX>`, where `HEX` is the
  upper-case hex of the lower-cased login. Hex uses only `0-9A-F`, so no owner
  can spell a filtered word. The owner `monkey` ended in `KEY` under the
  earlier `-`→`_` upper-casing scheme.
- **Git config.** Delivered through `GIT_CONFIG_PARAMETERS` (sq-quoted
  `'key=value'` entries, appended after any existing value), **not**
  `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_n`. The filter would keep the count and
  drop the keys, and every git command would then fail with "missing config
  key".
- **Manifest.** The owner list is `VK_GITHUB_ROUTED_OWNERS`, outside the
  `VK_GITHUB_PAT_` prefix. If a manifest shares a prefix with per-item names,
  some valid owner eventually collides with it: `owners` →
  `VK_GITHUB_PAT_OWNERS`.
- **Test.** A unit test asserts that no routing variable name contains a
  filtered word.

## Git: owner-scoped credential contexts

Verified with real git: `credential.https://github.com/<owner>.helper=` (an
empty value, which resets inherited helpers) followed by an inline `!f(){…}`
helper routes only URLs under that owner:

- `<owner>-other/…`, other owners, and host-only lookups keep the machine's
  existing helper.
- Command-scope config is read last, so the reset applies only to that URL
  prefix.
- Path matching is case-sensitive, so contexts are emitted for the owner as
  entered and in lowercase.
- The helper names the variable, not the value, so no token appears in any
  config text.

## `gh`: a shim prepended on the host that spawns

`<asset_dir>/github-auth/bin/gh` is a POSIX `sh` router. The executing host
(coordinator or worker) writes it atomically and **prepends** its directory to
PATH immediately before spawn. `apply_github_routing` does this at all four
boundaries: local execution, worker execution, local terminal, and worker
terminal. Two ordering traps:

- The CLI-tools directory is *appended* so that host tools win. The shim has
  to be *prepended* instead, or the real `gh` shadows it.
- `ExecutionEnv::with_profile` puts profile PATH entries first. Without
  `preserve_shim_precedence`, a profile that ships a `gh` silently bypasses
  routing.

Target precedence mirrors gh itself:

1. `-R`/`--repo`, including the `-RX` and `-R=X` forms.
2. A `gh api` endpoint with a literal owner. `repos/{owner}/{repo}`
   placeholders fall through, because gh fills them from the current
   repository. Absolute `api.github.com` endpoints count too, and
   `--hostname` overrides `GH_HOST` for a bare `OWNER/REPO` path.
3. A GitHub URL positional argument.
4. The repository positional of `gh repo SUB`. Only listed subcommands take
   one (`rename NEW-NAME` does not), and it must parse as a repository.
5. `GH_REPO`.
6. The current repository: `remote.<name>.gh-resolved` (written by `gh repo
   set-default`) first, then the push and upstream chain.

Argument parsing is the part that took twelve Codex review rounds to
converge:

- The values of value-taking flags must be consumed **before** option parsing.
  Otherwise `--body '--repo=x/y'` or `--body https://github.com/x/y` selects
  the wrong owner.
- Long flag names are unambiguous. Short letters are reused across commands:
  `pr review -a` approves and `pr merge -d/-m/-s` are booleans, but `-a`,
  `-m` and `-s` take values elsewhere. Short flags therefore need tables per
  command and subcommand.
- `--` ends option parsing, not target scanning.

A configured owner whose token is empty or unset fails closed with exit code
78, naming the owner. Unconfigured owners and other hosts pass through
unchanged.

## Testing recipe

- **Fake `gh`.** The next `gh` on PATH is a script that prints `GH_TOKEN`.
  Tests assert the token each argument shape receives.
- **Real Git.** Run `git credential fill` under a temporary `HOME` with
  `GIT_CONFIG_NOSYSTEM=1`, using the generated environment.
- **Other shells.** Run the shim under dash and busybox as well as bash. The
  agent sandbox's `/bin/sh` is bash, which accepts non-POSIX constructs.
- **Worker boundary.** A worker dispatch test proves the shim directory is
  first on PATH and that the Git parameters reach the child.

## Server-side operations: per-request credentials

The VK server's own git and `gh` calls (create, status, checks, merge and
update of PRs, pushes, rebase and status fetches, the PR monitor) used to run
with the process's ambient credential. On think2 that is a global
`credential.https://github.com.helper = gh auth git-credential`, so
`create_pr` got a 403 even though the org token could write. The shape of the
fix:

- **Split.** `utils::github_credentials` holds the pure part: owner parsing, a
  `GitHubCredentials` set, and `CredentialSelection` applied to one
  `Command`. `services::github_credentials` resolves the set per request
  (`resolve_for_repo` covers *every* remote, `resolve_for_urls` covers a PR
  URL). `git`/`git-host` cannot depend on services, so routes pass the set in.
  `GitService` network methods take `&GitHubCredentials`, which forces every
  call site to decide.
- **Freshness.** The table is re-read on every call. Only `op://` resolutions
  are cached: 60 s, keyed `(row id, updated_at, reference)`, cleared on
  Settings writes, and guarded by a generation counter so a lookup in flight
  cannot re-insert after an invalidation. Use the server's
  `OP_SERVICE_ACCOUNT_TOKEN`, because there is no workspace org here.
- **Fail closed.** No row means fallback. A row that cannot be decrypted or
  resolved, or a table read error, makes the owner *unavailable*, and the
  command is never spawned. Load the key file only when a requested owner has
  a row.
- **Never `Debug` a `Command`.** Its Debug output prints environment changes.
  `GitCli` traced it, which would have logged the token. Trace the args only.
- **Attribute auth failures**, for example "the sweetgreen org token … was
  rejected" or "no org token for owner X; used the server's fallback". Keep the
  original stderr in the message so a 403 is still classified
  `InsufficientPermissions`, which check coverage reads as `forbidden`.
- **Push skip.** `create_pr` checks `ls-remote` against the local tip, not a
  possibly stale `origin/<branch>`. Skip only when `pushInsteadOf` does not
  send pushes elsewhere.

### Git config can route around a credential helper (12 Codex rounds)

Installing an owner-scoped helper is not enough. Each of these bypassed it,
and each fix is command-scoped `GIT_CONFIG_PARAMETERS`:

- **URL form.** Use exactly `https://github.com/<owner>/<repo>.git`. SSH,
  `www.`, a port, `http://` and embedded userinfo all skip the helper
  context, so rewrite them for that command.
- **`url.*.insteadOf` / `pushInsteadOf`** (for example a host-wide
  https→ssh rule). Git takes the *longest* match and keeps the *first* of
  equally long ones. Identity rules at the owner and repository prefixes beat
  shorter rules. A rule exactly as long as the full URL still wins, so resolve
  the effective URL first (`ls-remote --get-url`, and `pushInsteadOf` by
  git's rule) and refuse if it moved.
- **`http.<url>.extraHeader`** (an Authorization header persisted by a CI
  checkout) outranks the helper. An empty value resets it. Emit the reset at
  the owner and repository URLs, because a more specific scope wins.
- **`git remote -v` and libgit2 `Remote::url()` report URLs *after*
  `insteadOf`.** A rewrite to an SSH host alias hides the owner. Use the
  configured `remote.<name>.url` (all scopes, `git remote -v` order), but
  only when it names a GitHub repo. An alias such as `gh:owner/repo` needs
  the expanded form.
- **`gh pr checkout` runs git itself.** Add SSH→HTTPS `insteadOf` for every
  SSH spelling, plus the owner spellings the checkout's remotes use (prefix
  matching is case-sensitive). Run the same effective-URL guard first, and
  check the owner too, not just the host.
- **Enterprise.** A GHE remote with a same-named owner must not get the
  github.com token.

Verify these with real git and no network: `git credential fill`,
`git ls-remote --get-url`, `git config --get-urlmatch http.extraheader <url>`,
and `git remote get-url --push`, all under a temporary `HOME` with
`GIT_CONFIG_NOSYSTEM=1`. Note that `git remote get-url` ignores remotes
defined only through `-c`.

## Contributed by

- vk/0f52-manage-gh-token
- vk/8b57-use-settings-git
