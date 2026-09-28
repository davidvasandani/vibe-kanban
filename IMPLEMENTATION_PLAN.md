# Implementation plan — vk/0f52-manage-gh-token

Spec: `SPEC.md`. Prior knowledge: `PRIOR_KNOWLEDGE.md`. SpecKit artifacts:
`homelab/specs/vk/0f52-manage-gh-token/`.

## 1. Storage (crates/db)

- Migration `20260928000000_github_owner_tokens.sql`:
  `github_owner_tokens(id BLOB PK, owner TEXT NOT NULL UNIQUE COLLATE NOCASE,
  encrypted_value TEXT NOT NULL, created_at, updated_at)`.
- Model `models/github_owner_token.rs` using **runtime** `sqlx::query_as`, not
  macros, like `mcp_gateway.rs`, so the `.sqlx` offline cache is unchanged.
  Functions: `list`, `find_by_id`, `create`, `update_value`, `delete`.

## 2. Domain service (crates/services `github_owner_tokens.rs`)

- `validate_owner`: 1–39 chars of `[A-Za-z0-9-]`, no leading or trailing
  hyphen.
- `normalize_value`: `normalize_secret_reference(v)`, or else the trimmed
  literal. Reject empty values, NUL, and values over 4 KiB.
- The envelope encryption reuses `McpGatewaySecretStore` with its own key file,
  `utils::assets::github_owner_tokens_key_path()`. The AAD binding is
  `vk-github-owner-token|<id>`.
- TS types: `GitHubOwnerToken { id, owner, reference?, created_at, updated_at }`,
  `CreateGitHubOwnerTokenRequest { owner, value }`, and
  `UpdateGitHubOwnerTokenRequest { value }`. The list decrypts rows and exposes
  only a normalized reference, never a literal.
- Errors (`GitHubOwnerTokenError`): InvalidOwner, DuplicateOwner, InvalidValue,
  NotFound, Store, Database, and `Resolve { owner, source }`. They are
  secret-safe; `Resolve` names the owner and wraps the `EnvironmentSecretError`
  category.
- `launch_environment(pool, org_env)`: decrypts every row, then resolves
  references for each owner with `environment_secrets::resolve_environment_secrets`
  on `{OP_SERVICE_ACCOUNT_TOKEN: <org literal if present>, KEY: value}`. The
  org's literal token is used first and the service env is the fallback, the
  same precedence as org Env Vars. It returns `VK_GITHUB_ROUTED_OWNERS` (owners
  as entered, comma-joined) plus `VK_GITHUB_PAT_<OWNER_KEY>` per owner. With
  no rows it returns an empty map. A decrypt failure is an error naming the
  owner.

## 3. Wire into launches (crates/local-deployment)

- `resolve_org_env_vars` becomes: org values (inner) → resolve org secrets →
  extend with `github_owner_tokens::launch_environment(pool, &raw_org)`. Every
  consumer already goes through it: local execution, worker dispatch, and both
  terminal routes. Because `VK_*` is reserved, org values cannot spoof these
  keys.
- `ContainerError::GitHubOwnerToken(#[from])` maps to `ApiError::BadRequest`
  with its message.
- If the store key cannot be read, execution also fails with a clear message.
  An empty table never touches the key file.

## 4. Spawn-side augmentation (crates/utils `github_auth.rs`)

- `GH_SHIM` is the embedded POSIX `sh` router. `ensure_github_shim_dir()`
  writes `<asset_dir>/github-auth/bin/gh`, mode 0755, atomically (temp file +
  rename), and rewrites it only when the content differs. It returns the
  directory.
- `github_routing_environment(get: impl Fn(&str) -> Option<String>)` returns
  `Option<Vec<(String, String)>>`. It returns `None` unless
  `VK_GITHUB_ROUTED_OWNERS` is non-empty. Otherwise it returns:
  - `PATH` = the shim directory prepended through `merge_paths`;
  - `GIT_CONFIG_COUNT` plus `GIT_CONFIG_KEY_n`/`VALUE_n`, continuing after
    any existing count taken from the env map or the inherited process env. For
    each owner variant (as entered, lowercased, deduped) this is a reset
    followed by an inline helper `!f(){ test "$1" = get || return 0; test -n
    "$VK_GITHUB_PAT_X" || return 0; printf 'username=x-access-token\n
    password=%s\n' "$VK_GITHUB_PAT_X"; }; f`. An invalid existing count means
    no git routing and a warning; the shim still applies.
- `apply_github_routing(env: &mut HashMap/BTreeMap)` is a thin wrapper used by
  all four spawn sites, each after its CLI-tools PATH step:
  1. `local-deployment/src/container.rs`, local execution (`env.vars`);
  2. `worker/src/execution.rs` `run_job`;
  3. `worker/src/terminal.rs`;
  4. `server/src/routes/terminal.rs`, local terminal branch.
  If the shim cannot be written, the error is logged and the launch goes on
  without the shim. Git routing still works, and `gh` falls back to ambient
  auth. The warning names the directory.

## 5. `gh` shim behavior (the script)

- Target precedence:
  1. `-R`/`--repo`/`--repo=` (scan stops at `--`);
  2. `gh api` endpoint `repos/OWNER/REPO…` (with or without a leading slash);
  3. a `https://github.com/OWNER/REPO…` URL argument;
  4. `gh repo <sub> OWNER/REPO` (the positional argument right after the
     subcommand);
  5. the cwd remote order (pushDefault → pushRemote → remote → origin → the
     only remote).
- Forms: `OWNER/REPO` counts as GitHub.com only when `GH_HOST` is unset or
  `github.com`. Also accepted: `github.com/OWNER/REPO`, the https URL,
  `ssh://git@github.com/`, and `git@github.com:`. Owner `[A-Za-z0-9-]`,
  compared lowercased.
- Membership comes from the `VK_GITHUB_ROUTED_OWNERS` list (compared without
  case). The token is read with `eval` over a validated key.
- If configured: an empty token exits 78 with the message
  `gh: configured GitHub token for <owner> is unavailable`. Otherwise
  `export GH_TOKEN`.
- The real `gh` is the first executable `gh` on `PATH` that is not the shim
  directory. If there is none, the shim exits 127.

## 6. HTTP API (crates/server `routes/github_owner_tokens.rs`)

- `GET /api/github-owner-tokens`, `POST` (create), `PUT /{id}` (replace the
  value), `DELETE /{id}`.
- Duplicate and invalid input returns 400 with an actionable message. A
  missing id returns 404.
- Types are registered in `generate_types.rs`; then run `pnpm run generate-types`.

## 7. Frontend (packages/web-core)

- `machineClient`: `listGitHubOwnerTokens`, `createGitHubOwnerToken`,
  `updateGitHubOwnerToken`, `deleteGitHubOwnerToken`.
- `GitHubOwnerTokensCard.tsx` is modeled on `OrganizationEnvVarsCard`: the same
  `normalizeSecretReference` draft, submit, and input-type helpers; masked
  literals; references shown. It sits in `ReposSettingsSection` at the bottom
  of the page, before the save bar, and is independent of the selected repo.
- i18n keys go in every locale that has the `settings.repos` block (see the
  locale-key-consistency KB page).
- Vitest: pure helpers (owner validation mirror, payloads) plus a rendered
  test of masked versus reference rows.

## 8. Docs

- `docs/`: a short section on GitHub organization tokens (where to find it,
  routing rules, precedence, rotation (applies to new processes), and the
  relationship to the homelab Nix router).
- `homelab/docs/vibe-kanban-github-auth.md`: point to the UI setting as the
  preferred path.

## 9. Verification

- `cargo test -p utils github_auth`: shim tests with a fake `gh` that prints
  `GH_TOKEN`, covering every precedence form, an unconfigured owner, an empty
  token (78), a missing real `gh` (127), and no recursion. Git tests run
  `git credential fill` with the generated `GIT_CONFIG_*` and a temporary HOME.
- `cargo test -p services github_owner_tokens`: validation, normalization, the
  encrypted round trip, list redaction, and launch env with a fake `op`.
- `cargo test -p db`, `-p worker`, `-p local-deployment`, and `-p server` for
  the touched tests. Then `pnpm run generate-types:check`, `pnpm run check`,
  `pnpm run lint`, the web-core vitest suite, and `pnpm run format`.
- Scan the diff for token-shaped strings; only synthetic tokens are allowed.

## Deliberate exception (constitution Governance)

Resolved PATs reach a cluster worker inside the authenticated dispatch
`environment`, as resolved org Env Vars already do (`vk/b0d4`). The older
Nix-router rule (no PATs in dispatch) assumed per-node Nix configuration. This
setting is UI-managed on the coordinator, and workers have no copy of it.
Sending references for workers to resolve would still need the literal values
to cross, and would add a second 1Password bootstrap on every worker.
