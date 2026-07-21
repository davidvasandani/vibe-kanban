# AWS SSO profile management

Tags: `6777-aws-sso-config-i`

## Vendor config files are edited, never owned

When VK manages entries in a config file owned by an external tool in the
user's home directory (`~/.aws/config` here), it acts as a guest editor
(constitution XIII). The implementation pattern that satisfies it
(`crates/services/src/services/aws_sso.rs`):

- A line-preserving document model: split the file into sections keyed by
  header (`[profile x]`, `[sso-session x]`, `[default]`, other), keep every
  line verbatim with its original ending, and serialize by concatenation.
  Rewriting a managed section regenerates only its managed keys and carries
  over unknown keys, comments, and continuation lines; everything outside the
  touched sections round-trips byte-for-byte (tests assert this).
- Refuse to write when the overall file cannot be parsed. Parse acceptance is
  deliberately narrow: headers must be `[...]`, top-level body lines must be
  `key = value`, comments, blanks, or indented continuations — anything else
  is a parse error, not a guess.
- Atomic replace: temp file in the same directory (`create_new` + `0600` on
  Unix), write + fsync, rename over the target. Create the parent directory
  `0700`. `std::fs::rename` replaces existing files on Windows too
  (`MoveFileExW` + `MOVEFILE_REPLACE_EXISTING`) — a review claim to the
  contrary was refuted.
- Serialize read-modify-write cycles behind one process-wide async mutex
  (`config_write_lock()`), or two concurrent saves both read the same
  original bytes and the last rename silently drops the other's write. Found
  by review, not by tests — concurrent mutation of a shared file needs an
  explicit lock even when each individual write is atomic.
- Only non-secret configuration is written; tokens stay in the vendor CLI's
  own storage (`~/.aws/sso/cache`). Sign-in is the vendor's own command in a
  PTY, per [cli-tool-oauth-login](cli-tool-oauth-login.md).

## AWS-specific shape

- **Write the modern sso-session form; read both.** `[sso-session <s>]`
  (with `sso_registration_scopes = sso:account:access` for refresh tokens)
  plus `[profile <name>]` referencing it. Legacy inline SSO profiles are
  listed and sign-in-able, and are converted to the modern form when edited.
- **Session derivation:** the prefix before the first `.` of the profile name
  (`ai-foundry.AdministratorAccess` → session `ai-foundry`), so `org.Role`
  families share one session and one cached token. Editing a shared session's
  start URL/region is rejected with a conflict naming the other referencing
  profiles — never silently forked.
- **`[default]` is list/sign-in only.** VK never rewrites the `[default]`
  section; `validate_profile_name(name, allow_default)` splits "valid login
  reference" from "valid writable name" — one uniform validator would either
  let writes target `default` or break sign-in for it.
- **Config path resolution must match the vendor's.** `AWS_CONFIG_FILE`
  first; then USERPROFILE on Windows / HOME on Unix, because Python's
  `expanduser` (which the AWS CLI uses) ignores HOME on Windows. Probes and
  the login PTY must inherit `AWS_CONFIG_FILE` and the Windows home vars
  explicitly — the minimal-env allowlists don't include them, and a login
  that verified the profile against one config file must not run against
  another.
- **Login lock is keyed by token-cache identity, not profile name:** the
  profile's `sso_session` value, else its inline `sso_start_url` (the legacy
  cache is keyed by URL), else the name. Two profiles sharing a session share
  a cache; concurrent logins would race the CLI's cache writes.
- **Auth probe:** `aws sts get-caller-identity --profile <name> --output
  json` under a whitelisted env (so ambient `AWS_*` credentials can't spoof
  a profile's status). Classify stderr conservatively: known
  expired/missing-token markers → unauthenticated; anything unrecognized →
  unknown, never authenticated. The real CLI's message
  ("Error loading SSO Token: Token for <session> does not exist") is covered
  by the `error loading sso token` marker.

## Route/UI integration

- The profile login WS (`crates/server/src/routes/aws.rs`) mirrors
  `cli_tools.rs::handle_login` with the same wire framing (`output` / `exit`
  / `status` / `error`), so the xterm.js terminal wiring transfers directly;
  only the `status` payload differs (`AwsSsoProfileStatus`).
- The generic CLI-tools catalog stayed untouched except its `Unsupported`
  message now points at the AWS section — runtime-parameterized commands do
  not fit the `&'static` catalog and live in a parallel additive module
  instead.

## Contributed by

- vk/6777-aws-sso-config-i
