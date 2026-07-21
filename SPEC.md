# Technical Specification: AWS SSO Profile Management in Vibe Kanban

Task: `6777-aws-sso-config-i`

## Problem

Coding agents and workspace processes launched by Vibe Kanban need
authenticated AWS access, but environments frequently ship with no
`~/.aws/config` and no configured SSO profile (e.g. the
`ai-foundry.AdministratorAccess` profile). Today there is no way to create,
inspect, or reauthenticate an AWS SSO profile from within Vibe Kanban:

- The managed CLI tool catalog (`crates/services/src/services/cli_tools.rs`)
  ships AWS CLI v2 as an installable tool, but its auth strategy is
  `CliToolAuthStrategy::Unsupported`, with the recorded rationale that
  "AWS SSO login requires choosing and configuring a profile."
- The knowledge base (`docs/knowledge-base/cli-tool-oauth-login.md`) confirms
  AWS SSO was deliberately deferred from the generic CLI login flow because it
  is profile-specific — the generic flow has no place for a runtime-chosen
  profile name.

The missing piece is a profile management layer: CRUD for AWS SSO profiles in
`~/.aws/config`, per-profile authentication status, and a per-profile
`aws sso login` flow. This must live in Vibe Kanban itself (backend + settings
UI), not in host provisioning (NixOS or otherwise).

## Goals

1. List AWS SSO profiles found in the host's AWS config file, with a typed
   per-profile authentication status.
2. Create, update, and delete SSO profiles (name, SSO start URL, SSO region,
   account ID, role name, default region, optional output format) from the
   settings UI, writing standard AWS CLI config syntax so the AWS CLI and SDKs
   consume the result directly.
3. Reauthenticate a profile from the UI by streaming an interactive
   `aws sso login --profile <name>` PTY session over the existing signed
   WebSocket infrastructure, so a user can complete the device/browser flow.
4. Report authentication truthfully: login-command exit and verified
   authentication are distinct facts; only an independent probe
   (`aws sts get-caller-identity --profile <name>`) confirms success.
5. Keep all of this machine-scoped: operations act on the host selected in the
   settings dialog, via the existing machine-aware client and signed routes.

## Non-Goals

- Storing AWS credentials or tokens inside Vibe Kanban. VK orchestrates the
  vendor CLI; tokens stay in the CLI's own storage (`~/.aws/sso/cache`).
  This is the boundary mandated by `docs/knowledge-base/cli-tool-oauth-login.md`.
- Managing static IAM access keys, `credential_process` entries, or
  `~/.aws/credentials`. SSO profiles only.
- Becoming an OAuth/OIDC client (no direct device-authorization flow).
- Remote deployment (`crates/remote`) support; this is a host-machine feature
  surfaced in the local server, same as CLI tools.
- Shipping hard-coded organization defaults (e.g. a committed
  `ai-foundry.AdministratorAccess` start URL). The feature enables users to
  provision that profile; it does not embed tenant-specific values in the repo.

## Background / Existing Infrastructure to Reuse

| Concern | Existing mechanism |
| --- | --- |
| Interactive vendor login in a terminal | PTY sessions via `deployment.pty().create_command_session(...)`, streamed over signed WS in `crates/server/src/routes/cli_tools.rs` (`handle_login`), 15-min timeout, one-login lock |
| One-shot probes | `tokio::process::Command` with `kill_on_drop`, timeout, and minimal whitelisted env (`probe_auth` in `crates/services/src/services/cli_tools.rs`) |
| AWS binary resolution | `effective_binary` for `CliToolId::Aws` — host `aws` on PATH takes precedence over the app-managed copy under `cli_tools_bin_dir()` |
| Route registration | `pub fn router() -> Router<DeploymentImpl>` merged into the relay-signed router chain in `crates/server/src/routes/mod.rs` |
| Shared types | `#[derive(TS)]` + decl entries in `crates/server/src/bin/generate_types.rs`, regenerated with `pnpm run generate-types` |
| Settings UI registration | Table-driven `settingsRegistry.tsx` in `packages/web-core/src/shared/dialogs/settings/settings/` (`SettingsSectionType`, `SETTINGS_SECTION_DEFINITIONS`, `renderSettingsSection`) |
| Machine-scoped API calls | `MachineClient` (`packages/web-core/src/shared/lib/machineClient.ts`) obtained via `useSettingsMachineClient()`; WS via `openLocalApiWebSocket` with machine request options |
| Embedded login terminal | xterm.js pattern in `CliToolsSettingsSection.tsx` |
| Profile-list CRUD form UX | `OrganizationEnvVarsCard.tsx` add/edit/delete entry pattern |

## Design

### Data model

New service module `crates/services/src/services/aws_sso.rs` (registered in the
services module tree) owning the AWS config file representation:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AwsSsoProfile {
    pub name: String,           // e.g. "ai-foundry.AdministratorAccess"
    pub sso_start_url: String,  // https://<org>.awsapps.com/start (or vanity URL)
    pub sso_region: String,     // region of the SSO directory, e.g. "us-east-1"
    pub sso_account_id: String, // 12-digit account id
    pub sso_role_name: String,  // permission-set role name
    pub region: Option<String>, // default client region for the profile
    pub output: Option<String>, // json | yaml | text | table
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum AwsAuthStatus {
    Authenticated { identity: String }, // caller-identity Arn
    Unauthenticated,                    // probe ran, not logged in / expired
    Unknown { message: String },        // probe failed to run / timed out
    CliMissing,                         // no aws binary resolvable
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AwsSsoProfileStatus {
    pub profile: AwsSsoProfile,
    pub auth: AwsAuthStatus,
}
```

### AWS config file handling

- **Location:** honor `AWS_CONFIG_FILE` if set in the server process
  environment; otherwise `$HOME/.aws/config`. Create the `~/.aws` directory
  (mode `0700`) and the config file (mode `0600` on Unix) if absent.
- **Format written (modern sso-session form):** for each managed profile VK
  writes a `[profile <name>]` section referencing an `[sso-session <name>]`
  section:

  ```ini
  [sso-session ai-foundry]
  sso_start_url = https://ai-foundry.awsapps.com/start
  sso_region = us-east-1
  sso_registration_scopes = sso:account:access

  [profile ai-foundry.AdministratorAccess]
  sso_session = ai-foundry
  sso_account_id = 123456789012
  sso_role_name = AdministratorAccess
  region = us-east-1
  output = json
  ```

  The session name is the profile name's prefix before the first `.` when one
  exists (so profiles like `ai-foundry.AdministratorAccess` and
  `ai-foundry.ReadOnly` share one session/token), else the profile name
  itself. When an sso-session section of that name already exists with a
  different `sso_start_url`/`sso_region`, the write is rejected with a
  conflict error naming the other profiles referencing that session, unless
  the caller is updating every referencing profile to the same values.
- **Format read:** both modern (`sso_session` reference) and legacy inline
  (`sso_start_url` directly in the profile section) SSO profiles are parsed
  and listed. Profiles without SSO keys (static keys, `credential_process`,
  `[default]` without SSO) are ignored by the list endpoint.
- **Round-trip safety:** the parser is a conservative line-based INI editor:
  unknown sections, unknown keys, comments, and ordering outside the touched
  sections are preserved byte-for-byte. Writes replace only the affected
  `[profile <name>]` / `[sso-session <x>]` sections. A failed parse of the
  overall file structure aborts the write with an error (never clobber a file
  we cannot understand). Writes go through a temp file + atomic rename.
- **Delete:** removes the `[profile <name>]` section; removes its
  `[sso-session]` section only when no remaining profile references it.

### Validation (server-side, before any write)

- `name`: non-empty, ≤ 128 chars, matches `^[A-Za-z0-9_.@-]+$` (no
  whitespace, brackets, or control characters — these would corrupt section
  headers or shell args). The name is the only user-controlled value
  interpolated into command args, so this doubles as command-injection
  defense (args are passed as a vector, never a shell string, per the KB rule
  "never accept a command string from the browser").
- `sso_start_url`: `https://` URL.
- `sso_region` / `region`: `^[a-z]{2}(-[a-z]+)+-\d$` (e.g. `us-east-1`).
- `sso_account_id`: exactly 12 ASCII digits.
- `sso_role_name`: non-empty, ≤ 64 chars, `^[A-Za-z0-9+=,.@_-]+$`.
- `output`: one of `json`, `yaml`, `text`, `table` when present.

### API surface

New route module `crates/server/src/routes/aws.rs`, merged into the
relay-signed router in `crates/server/src/routes/mod.rs`:

| Method & path | Behavior |
| --- | --- |
| `GET /api/aws/profiles` | Parse config file, probe each SSO profile's auth concurrently (short timeout), return `Vec<AwsSsoProfileStatus>` |
| `PUT /api/aws/profiles/{name}` | Upsert one profile from an `AwsSsoProfile` body (path name must equal body name); returns the saved profile |
| `DELETE /api/aws/profiles/{name}` | Remove the profile (404 if absent or not an SSO profile) |
| `GET /api/aws/profiles/{name}/login/ws` | Signed WebSocket; runs `aws sso login --profile <name>` in a PTY and streams bytes both ways |

All JSON endpoints return the standard `ApiResponse<T>` envelope. The login WS
uses `SignedWsUpgrade` exactly like `cli_tools.rs::login_cli_tool`.

### Auth probe

`aws sts get-caller-identity --profile <name> --output json`, executed with:

- the effective AWS binary (host copy preferred, then managed copy; if
  neither resolves, status is `CliMissing` and no probe is spawned),
- `kill_on_drop(true)` and a short timeout (reuse the existing probe timeout
  constant),
- a minimal environment (whitelist `HOME`, `USER`, `PATH`, `TMPDIR`, `LANG`,
  plus `AWS_CONFIG_FILE`/`AWS_SHARED_CREDENTIALS_FILE` if set) so stray
  `AWS_*` credentials in the server env cannot fake a profile's status.

Exit 0 with parseable JSON → `Authenticated { identity: <Arn> }`. Non-zero
exit whose stderr indicates expired/absent SSO token → `Unauthenticated`.
Anything else (timeout, spawn failure, unparseable output) → `Unknown`.

### Login flow (PTY over signed WS)

Mirrors `cli_tools.rs::handle_login` with a per-profile lock:

1. Validate the profile name and confirm the profile exists in the config
   file and the AWS binary resolves; otherwise close the WS with a typed
   failure message.
2. Enforce one active login per profile name per server process
   (`try_begin_login`-style guard), and the existing 15-minute
   `LOGIN_TIMEOUT`.
3. Spawn `aws sso login --profile <name>` via
   `deployment.pty().create_command_session(...)` with working dir `$HOME`,
   so tokens land in `~/.aws/sso/cache`.
4. Stream base64 PTY bytes both directions; never persist or log transcripts.
5. On child exit: run the auth probe. Only a confirming probe yields the
   `success` terminal message; a zero exit with a failing probe reports
   `exit-without-auth`. Cancel/timeout/disconnect kill the child via the
   cloned killer; after normal reap, remove the session without signalling
   (PTY-lifecycle lesson from the knowledge base).

### CLI Tools catalog touch-up

`CliToolId::Aws` keeps `CliToolAuthStrategy::Unsupported`, but its message is
updated to point at the new AWS section ("Manage SSO profiles and sign in from
the AWS section in Settings"). No catalog structure changes.

### Frontend

All in `packages/web-core/src/shared/dialogs/settings/settings/`:

1. **Registry:** add `'aws'` to `SettingsSectionType`, an entry in
   `SettingsSectionInitialState`, a `SETTINGS_SECTION_DEFINITIONS` row
   (group `'host'`, cloud icon), and a `renderSettingsSection` case.
2. **`AwsSettingsSection.tsx`:**
   - Fetches `listAwsProfiles()` through `useSettingsMachineClient()`; shows
     an install hint linking to the CLI Tools section when status is
     `CliMissing`.
   - Profile list rows: name, account/role summary, auth status badge
     (Authenticated / Not signed in / Unknown), actions: **Sign in**
     (or **Reauthenticate**), **Edit**, **Delete** (with confirm).
   - Add/Edit form (modal or inline card following
     `OrganizationEnvVarsCard.tsx`): fields for the seven profile values with
     client-side mirrors of the server validation and server-error surfacing.
   - **Sign in** opens the embedded xterm.js terminal (same wiring as
     `CliToolsSettingsSection.tsx`: FitAddon, WebLinksAddon so the device
     URL is clickable, `error`/premature-`close` handling so a rejected
     upgrade never leaves the terminal stuck "running"), then refreshes the
     profile list when the socket reports a terminal outcome.
3. **`machineClient.ts`:** add `listAwsProfiles`, `saveAwsProfile`,
   `deleteAwsProfile`, `openAwsProfileLogin` to the `MachineClient` interface
   and implementation, typed with the generated `AwsSsoProfileStatus` /
   `AwsSsoProfile` types.

### Type generation

Add `AwsSsoProfile`, `AwsAuthStatus`, `AwsSsoProfileStatus` (and any
request/error enums) decls to `crates/server/src/bin/generate_types.rs`; run
`pnpm run generate-types`. `shared/types.ts` is never hand-edited.

## Security Considerations

- No secrets stored or proxied by VK; tokens remain in `~/.aws/sso/cache`
  owned by the AWS CLI.
- Executable + argument vectors are built entirely server-side from the
  compiled catalog and the validated profile name; the browser never supplies
  a command string.
- Profile names are strictly validated before use in file sections, paths, or
  arg vectors.
- Probes run with a minimal env so ambient `AWS_*` variables can't spoof
  status; login PTY inherits the normal login-shell env it needs for the
  browser handoff.
- Config file written `0600`, directory `0700`, atomic rename; unparseable
  files are never rewritten.
- All routes sit behind the existing relay signature middleware; the login WS
  is signed.

## Testing

- **Rust (`services::aws_sso`):** parser round-trips (modern + legacy
  profiles, comments/unknown sections preserved, mixed files), upsert into
  empty/missing file, sso-session sharing and conflict rejection, delete with
  and without remaining session references, all validation rules, probe
  result classification from canned outputs. `cargo test -p services`.
- **Rust (routes):** name-validation rejection, path/body name mismatch,
  per-profile login lock (no real login spawned), 404 delete.
- **Frontend:** pure state-mapping tests for status→action visibility
  (Sign in vs Reauthenticate vs install hint), following the existing
  `cliToolLogin.ts` test pattern; `pnpm run check` and `pnpm run lint`.
- **Generated types:** `pnpm run generate-types:check`.
- **Manual:** create a profile, run Sign in against a real SSO start URL,
  confirm `aws sts get-caller-identity --profile <name>` succeeds and the
  badge flips to Authenticated.

## Acceptance Criteria

1. With no `~/.aws` present, a user can open Settings → AWS, create the
   `ai-foundry.AdministratorAccess` profile by entering its SSO parameters,
   and the resulting `~/.aws/config` is valid for
   `aws sso login --profile ai-foundry.AdministratorAccess`.
2. The profile list shows each SSO profile with a truthful auth status, and
   Sign in completes the device flow in the embedded terminal, after which
   the status shows Authenticated with the caller identity.
3. Editing and deleting profiles never disturbs unrelated content in
   `~/.aws/config`.
4. `pnpm run check`, `pnpm run lint`, `cargo test --workspace` (touched
   crates), and `pnpm run generate-types:check` pass.
