# Implementation Plan: AWS SSO Profile Management in Vibe Kanban

Task: `6777-aws-sso-config-i` — see `SPEC.md` for the full design and
`PRIOR_KNOWLEDGE.md` for the constraints inherited from the knowledge base.

## Backend

1. **`crates/services/src/services/aws_sso.rs` (new):**
   a. Conservative line-based AWS config INI editor: parse sections
      (`[profile x]`, `[sso-session x]`, `[default]`, unknown), preserve
      untouched lines byte-for-byte, atomic temp-file + rename writes,
      `0700`/`0600` permissions on create, honor `AWS_CONFIG_FILE`.
   b. `AwsSsoProfile` / `AwsAuthStatus` / `AwsSsoProfileStatus` types with
      `#[derive(TS)]`.
   c. `list_profiles()` reading modern (`sso_session`) and legacy inline SSO
      profiles; `upsert_profile()` writing sso-session form with shared
      session derivation (prefix before first `.`) and session conflict
      rejection; `delete_profile()` with reference-counted session cleanup.
   d. Field validation (name charset/length, https start URL, region regex,
      12-digit account id, role-name charset, output enum).
   e. Auth probe: `aws sts get-caller-identity --profile <name> --output
      json` with effective-binary resolution reused from `cli_tools`
      (host `aws` first, then managed copy → else `CliMissing`), probe
      timeout, `kill_on_drop`, minimal whitelisted env; classify exit/output
      into `Authenticated{identity}` / `Unauthenticated` / `Unknown`.
   f. Unit tests: parser round-trips (modern/legacy/mixed/comments), upsert
      into missing file, session sharing + conflict, delete cleanup,
      validation matrix, probe-output classification from canned data.

2. **`crates/server/src/routes/aws.rs` (new):**
   a. `GET /api/aws/profiles` — list + concurrent probes →
      `Vec<AwsSsoProfileStatus>`.
   b. `PUT /api/aws/profiles/{name}` — validate, path/body name equality,
      upsert.
   c. `DELETE /api/aws/profiles/{name}` — 404 when absent.
   d. `GET /api/aws/profiles/{name}/login/ws` — `SignedWsUpgrade`; per-profile
      login lock, 15-min timeout, PTY `aws sso login --profile <name>` with
      `$HOME` working dir via `deployment.pty().create_command_session(...)`;
      on child exit run the auth probe and emit distinct
      success / exit-without-auth outcomes; cancel/timeout/disconnect kill via
      cloned killer, no signalling after normal reap (mirror
      `cli_tools.rs::handle_login`).
   e. Register `pub mod aws;` and `.merge(aws::router())` in
      `crates/server/src/routes/mod.rs`.
   f. Route tests: invalid-name rejection, name mismatch, login-lock
      exclusivity without spawning a real login.

3. **Catalog touch-up:** update the `CliToolId::Aws`
   `CliToolAuthStrategy::Unsupported` message in
   `crates/services/src/services/cli_tools.rs` to point at the new AWS
   settings section.

4. **Generated types:** add the three AWS type decls to
   `crates/server/src/bin/generate_types.rs`; run
   `pnpm run generate-types`.

## Frontend (`packages/web-core`)

5. **`machineClient.ts`:** add `listAwsProfiles`, `saveAwsProfile`,
   `deleteAwsProfile`, `openAwsProfileLogin` (WS) to the `MachineClient`
   interface + implementation using the generated types.

6. **Settings registry:** add the `'aws'` section type, initial-state entry,
   `SETTINGS_SECTION_DEFINITIONS` row (group `'host'`), and
   `renderSettingsSection` case.

7. **`AwsSettingsSection.tsx` (new):**
   a. Profile list with auth badges and Sign in / Reauthenticate / Edit /
      Delete actions; `CliMissing` install hint pointing at CLI Tools.
   b. Add/Edit form following `OrganizationEnvVarsCard.tsx`, mirroring
      server validation and surfacing server errors.
   c. Embedded xterm.js login terminal cloned from
      `CliToolsSettingsSection.tsx` (FitAddon, WebLinksAddon, `error` +
      premature-`close` handling); refresh list on terminal outcome.
   d. Pure state-mapping helper (status → available action) with a
      lightweight test, following the `cliToolLogin.ts` pattern.

## Verification

8. `cargo test -p services -p server` (new suites), `pnpm run
   generate-types:check`, `pnpm run check`, `pnpm run lint`,
   `pnpm run format`, `git diff --check`.
9. Manual smoke: with a scratch `AWS_CONFIG_FILE`, create
   `ai-foundry.AdministratorAccess`, verify the written INI, confirm the AWS
   CLI accepts `aws configure list-profiles`, and exercise the login WS
   opening path (real SSO completion requires interactive browser access).
10. Independent Codex diff review; address confirmed findings and re-verify
    until it reports no significant findings.
11. Distill reusable knowledge (AWS config round-trip editing, profile-scoped
    login-lock pattern) into the project knowledge base, refresh indexes, and
    commit.
