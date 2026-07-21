# Prior Knowledge: AWS SSO Profile Management in VK

Task: `6777-aws-sso-config-i`

The project knowledge base was searched via `docs/knowledge-base/INDEX.md` and
`wiki/INDEX.md` (plus a grep for AWS/SSO across `docs/`, `wiki/`, and
`specs/`) for pages about CLI tool management, vendor login flows, settings
UI, environment handling at process boundaries, and anything AWS-specific.
Four pages are directly relevant; the rest of the KB covers unrelated
subsystems (kanban UI, Electric sync, Slack/Jira connectors, agent process
lifecycle).

## CLI tool OAuth login (`docs/knowledge-base/cli-tool-oauth-login.md`)

The design bible for this feature — it documents the prior decision that
**directly created this task's gap**:

- Boundary rule: VK must orchestrate the vendor CLI, never become an OAuth
  client or credential store. Launch the durable login command in a PTY;
  tokens stay in the CLI's own host-side storage. In-app login is offered
  only when (1) credentials survive the login child process and (2) a
  separate non-secret command independently verifies auth. AWS SSO satisfies
  both — `aws sso login` persists tokens in `~/.aws/sso/cache`, and
  `aws sts get-caller-identity --profile <name>` verifies — but was excluded
  from the generic flow "because it is profile-specific." This feature adds
  the missing profile-selection layer.
- Backend rules to inherit: executable and args live in compiled server code
  (never accept a command string from the browser); resolve the effective
  binary the same way agent execution does (host binary beats app-managed
  copy); probes run concurrently with short timeout, `kill_on_drop`, minimal
  env, and typed authenticated/unauthenticated/unknown/unsupported results;
  use the signed WebSocket + machine-aware routing (a path alone can target
  the UI machine by mistake); one active login per target per server process
  with a maximum session duration; stream PTY bytes only, no transcripts;
  command exit and verified auth are distinct facts — zero exit becomes
  success only after the independent probe confirms.
- PTY lifecycle lesson: direct command sessions need an exit channel and a
  cloned child killer; cancel/timeout/disconnect kill and remove the child,
  but after normal reap remove the session **without** signalling the cloned
  PID (rapid PID reuse hazard). Browser side must handle `error` and
  premature `close` — a WebSocket constructor can succeed before its HTTP
  upgrade is rejected, which otherwise leaves the terminal stuck "running."
- Validation pattern: test catalog eligibility and concurrent locks without
  real logins; test a harmless direct PTY command; test frontend action
  visibility as a pure state mapping.

## Managed CLI tool catalog (`wiki/managed-cli-tool-catalog.md`)

- The catalog in `crates/services/src/services/cli_tools.rs` already ships
  AWS CLI v2 (`CliToolId::Aws`, wire id `"aws"`, Linux x86_64/aarch64 pinned
  sources, macOS intentionally unsupported) with
  `auth: Unsupported("AWS SSO login requires choosing and configuring a
  profile...")`. This feature builds beside the catalog rather than extending
  `CliToolAuthStrategy` — the static `&'static [&'static str]` args have no
  slot for a runtime profile name.
- Routes and settings UI consume the catalog generically; keep them generic
  and put AWS-profile-specific behavior in new modules.
- Never hand-edit `shared/types.ts`; add decls to
  `crates/server/src/bin/generate_types.rs` and run
  `pnpm run generate-types` (`generate-types:check` is enforced in CI).
- Validation sequence precedent: `cargo test -p services cli_tools`,
  `pnpm run generate-types:check`, repo checks, `pnpm run format`.

## Workspace environment inheritance (`docs/knowledge-base/workspace-environment-inheritance.md`)

- Environment must be an explicit choice at every child-process boundary.
  Managed CLI login PTYs deliberately pass an empty workspace map and keep a
  minimal allowlisted host environment — the AWS login PTY and the auth
  probe should follow the same discipline (whitelisted env for probes so
  ambient `AWS_*` vars can't spoof status).
- Never mutate the long-lived server environment or write secret files as a
  side channel.

## Prior speckit feature precedent (`specs/003-cli-tool-oauth-login/`)

The CLI-tool login feature that this task extends was itself developed
through speckit (spec/research/contracts under
`specs/003-cli-tool-oauth-login/`), and
`specs/vk/fc47-atlassian-cli-to/contracts/cli-tools.md` documents the
CLI-tools HTTP contract shape to mirror for new `/api/aws/*` endpoints.
Existing feature directories use both `NNN-slug` and `vk/<task-id>` naming;
the highest numbered prefix in use is `003`.

## Non-KB repo facts confirmed during recall

- No `~/.aws/config` parser/writer exists anywhere in the repo — greenfield.
- Settings sections are table-driven in
  `packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx`;
  machine-scoped calls go through `MachineClient`
  (`packages/web-core/src/shared/lib/machineClient.ts`) obtained via
  `useSettingsMachineClient()` from `SettingsHostContext.tsx`.
- `CliToolsSettingsSection.tsx` contains the reusable xterm.js login-terminal
  wiring; `OrganizationEnvVarsCard.tsx` is the closest add/edit/delete list
  form precedent.
