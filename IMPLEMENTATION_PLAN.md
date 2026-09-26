# Implementation plan: unattended re-authentication (`vk/ba8f-auth-auth`)

See `SPEC.md` for requirements and `PRIOR_KNOWLEDGE.md` for context. Steps are
ordered by dependency. Steps with the same number letter (3a/3b/3c) are
independent of one another.

## 1. Generalize the Entra driver (`crates/services/src/services/entra_mint.rs`)

1. Make `BrowserSession` usable by sibling modules (`pub(crate)`): `open`,
   `navigate`, `execute`, `url`, `close`. Add `open_default(cfg, save)`,
   which builds its own HTTP client.
2. Extract the step loop of `interactive_sign_in` into
   `pub(crate) async fn drive<T>(cfg, http, session, progress, timeout,
   on_page) -> Result<T, EntraError>`:
   - On every tick it runs `PROBE_JS`. When the page is an Entra step
     (`classify`), it performs that step, reusing today's selectors.
   - Otherwise it calls `on_page(&Probe) -> PageVerdict<T>`, which returns
     `Continue`, `Done(T)` or `Fail(String)`.
   - `interactive_sign_in` becomes `drive(...)` with a nativeclient-redirect
     verdict. Behaviour is unchanged, and the existing `classify` tests keep
     passing.
   - New `Step::WrongPassword` (text "your account or password is
     incorrect" / "password is incorrect") aborts at once with a named error.
     Retrying could lock the account.
   - New `Step::FlowTimedOut` ("session has timed out", "request is timed
     out") returns `EntraError::FlowTimeout`, so callers can retry with a
     fresh flow.
   - `Probe` carries `url`, `txt`, `has_email` and `has_otc`, plus a new
     `has_password` (`input[name="passwd"]` visible). Pure
     `fn classify_probe(&Probe)` keeps it testable.
3. **1Password references through Connect.** Add `OnePasswordConnect
   { host, token }::from_env()` (the same env/credential lookup `EntraConfig`
   uses; `EntraConfig.op` embeds it). Then:
   - `parse_op_ref("op://vault/item/field")` returns `OpRef`. It is pure and
     rejects empty segments, extra segments and a missing scheme.
   - `resolve_op_ref(http, connect, &OpRef)` does three lookups:
     `GET /v1/vaults?filter=name eq "<v>"` (or the id when it is 26
     lowercase alphanumeric characters), then `GET /v1/vaults/{v}/items?
     filter=title eq "<i>"` (same id rule), then the item. The field matches
     by `id` first, then case-insensitive `label`. It returns `totp` when
     present, else `value`. The error names the reference, never the value.
   - Pure JSON selectors (`select_field(item_json, name)`) keep the logic
     testable without HTTP.
4. `pub(crate) fn redact_url(url) -> String` keeps scheme, host and path and
   drops the query and fragment. Unit tested.

## 2. Re-auth core (`crates/services/src/services/reauth/`)

- `mod.rs`:
  - `ReauthTargetId` enum with Display/FromStr: `AwsSession(name)`,
    `AwsProfile(name)`, `CliTool(CliToolId)`, `Sgsc(backend)`. Parsing
    validates names with the existing `aws_sso` validators, plus
    `[a-z0-9_-]{1,32}` for backends.
  - `ReauthKind`, `ReauthAuthState` (`authenticated | unauthenticated |
    unknown | not_configured`) and `ReauthRunOutcome` (`running | succeeded
    | failed | verification_failed`).
  - `ReauthRun { started_at, finished_at, outcome, message, transcript:
    Vec<String> }`.
  - `ReauthTargetStatus { id, kind, label, swept, auth_state, auth_message,
    last_run }`.
  - `ReauthOverview { sweep_interval_secs: Option<u64>, targets }`. All carry
    `#[derive(TS)]`.
  - A process-wide registry (`OnceLock<Mutex<HashMap<String, TargetState>>>`)
    holds the last run, failure count and next-eligible time.
  - `list_targets()` builds targets from the existing sources:
    - CLI tools whose strategy is unattended (`EntraMint`,
      `EntraNativeBrowser`, new `ApiToken`).
    - AWS auth scopes from `aws_sso::list_profile_statuses()`, grouped by
      scope (session target when `session_name` is set, else profile). The
      aggregate state is unauthenticated if any profile is, authenticated if
      all are, and unknown otherwise.
    - `sgsc:<b>` for each backend in `VK_SGSC_BACKENDS`.
  - `start(target) -> ReauthRun`: claims the target under the registry lock
    and spawns a detached `tokio` task, the request-independent owner. If the
    target is already running it returns the running state
    (`already_running`) and does not spawn. The task holds the vendor-level
    lock too (`aws_sso::try_begin_profile_login(lock_key)` /
    `cli_tools::try_begin_login(id)`), so a manual PTY sign-in cannot race
    it. `wait(target, max)` polls the registry for up to `max`.
  - `run_one(target, progress)` dispatches to the engines. A `Progress`
    collector appends redacted lines to the transcript, capped at 200 lines.
  - Success is always re-verified with the independent probe:
    `cli_tools::status` for tools and `aws_sso::profile_status` for every
    profile in the scope.
- `aws.rs`:
  - Pure `parse_device_prompt(output) -> Option<DeviceUrl>` prefers a URL
    containing `user_code=`, else a URL plus a code (`[A-Z0-9]{4}-[A-Z0-9]{4}`)
    joined as `?user_code=`.
  - Pure `aws_page_verdict(&Probe)` returns: "Request approved" or "you can
    close this window" → `Done`; a visible *Confirm and continue*, *Allow
    access* or *Allow* button → click; otherwise continue.
  - `run(scope)`:
    1. Build the command through the existing
       `aws_sso::login_command_for_{session,profile}`, adding `--no-browser`
       and piping stdout/stderr (not a PTY), with `kill_on_drop`.
    2. Scrape the URL within 30s and open a firecrawl session on the Entra
       profile (`save=true`).
    3. `drive()` until the verdict is done, the child exits 0, or 4 minutes
       pass. Then wait up to 30s for the child.
    4. On `FlowTimeout` or a rejected code, retry up to 3 times with a fresh
       child.
- `acli.rs`:
  - `ApiTokenConfig::from_env()` reads `VK_ATLASSIAN_SITE`,
    `VK_ATLASSIAN_EMAIL_REF`, `VK_ATLASSIAN_TOKEN_REF` and
    `VK_ATLASSIAN_VERIFY_PAGE_ID`.
  - `run()` resolves the refs through Connect and runs `acli confluence auth
    login --site S --email E --token`, writing the token and `\n` to stdin.
    stdout/stderr are sanitized: every occurrence of the token is replaced
    before any line reaches the transcript.
  - Verification is the catalog probe `confluence page view --id <id> --json`.
- `sgsc.rs`:
  - Pure `onboard_url(template, backend)` requires the `{backend}`
    placeholder.
  - Pure `sgsc_page_verdict(&Probe, backend)` returns:
    - `Done` when the URL path contains `/_sgsc/auth/callback/<backend>`, or
      the gateway host's text says "connected" but not "connect … to
      continue".
    - Click for the Snowflake `ENTRA_ID_SSO` / "Sign in using" button.
    - Click for the Salesforce `Allow` button.
    - `NeedsMyApps` for a Salesforce native login form.
  - `run(backend)` navigates, drives, and on `NeedsMyApps` does this once:
    navigate to `https://myapps.microsoft.com`, click the Salesforce tile
    (Playwright, awaiting the popup page), wait 8s, then navigate to the
    onboard URL again. The run is limited to 3 attempts on `FlowTimeout`.
- `sweep.rs`:
  - Pure `next_eligible(now, failures, interval) = now + min(interval ×
    2^(failures−1), 6h)`.
  - Pure `sweep_interval_from_env()` returns `None` when the variable is
    unset or 0, and clamps other values to at least 300s.
  - `spawn_sweep(shutdown)` starts a loop with an initial delay of 120s. Each
    tick lists targets where `swept && auth_state == unauthenticated &&
    eligible`, runs them sequentially through `start` + `wait`, and logs each
    target id and outcome at `info!`.

## 3a. CLI catalog: acli becomes unattended (`cli_tools.rs`)

- New `CliToolAuthStrategy::ApiToken { probe: fn() -> Result<Vec<String>,
  String> }`. For acli it builds `confluence page view --id <page> --json`
  from `VK_ATLASSIAN_VERIFY_PAGE_ID`, or returns `Err("VK_ATLASSIAN_* not
  configured")`. That error maps to `Unsupported` with the reason, matching
  today's UX when it is not configured.
- `probe_args_of` becomes owned (`Vec<String>`). `login_plan` gains
  `CliToolLoginPlan::ApiToken`. `login_supported` includes it when
  configured.
- Existing tests are updated, and new tests cover the acli strategy and the
  probe args.

## 3b. Server routes (`crates/server/src/routes/reauth.rs`, `routes/cli_tools.rs`)

- `GET /reauth/targets` returns `ReauthOverview`.
- `POST /reauth/run` takes `{ target?: string, wait_secs?: u32 }` (≤ 55;
  default 0). With no target it starts every swept, unauthenticated
  target. It returns `Vec<ReauthTargetStatus>`, and an invalid target id
  gives `400`.
- Merge into `routes/mod.rs`.
- In `cli_tools.rs`, add a third `EntraFlow::ApiToken` variant that calls
  `reauth::acli::run` with the socket `Progress`, so the existing
  Authenticate button works for acli.
- Register the new types in `crates/server/src/bin/generate_types.rs` and
  run `pnpm run generate-types`.

## 3c. MCP tools (`crates/mcp/src/task_server/tools/reauth.rs`)

- `list_reauth_targets` calls `GET /api/reauth/targets`.
- `reauthenticate { target?: string }` calls `POST /api/reauth/run` with
  `wait_secs: 50`. It returns the states plus a hint to call
  `list_reauth_targets` if a run is still `running`. The description names
  the triggering errors (expired SSO token, `az` `InteractionRequired`, acli
  `unauthorized`, sgsc `reauth_required`).
- Add both to the global and orchestrator routers, and update the exact-set
  router test.

## 4. Deployment wiring

- In `crates/local-deployment/src/lib.rs`, next to `PrMonitorService::spawn`,
  call `services::services::reauth::sweep::spawn_sweep(shutdown.child_token())`.
  It is a no-op when the variable is unset.

## 5. Settings UI (`packages/web-core`)

- `machineClient.ts` gains `listReauthTargets()` and `runReauth(target?)`.
- New `ReauthSettingsCard.tsx`, rendered under the CLI tools card in
  `CliToolsSettingsSection.tsx`:
  - It uses `SettingsCard`.
  - Each target row shows the label, a state badge and the last outcome and
    message, kept verbatim (constitution XI).
  - Each row has a *Re-authenticate* button, and the card has
    *Re-authenticate all*.
  - It polls every 3s while any run is `running`, and shows the sweep
    interval or "off".
  - An expandable transcript is shown in a `<pre>`.
- Add i18n keys to every settings locale (the English strings are copied
  where no translation exists, following the repo's key-consistency gate).
- `ReauthSettingsCard.test.tsx` covers the list render, a run click that
  calls the client and shows running then succeeded, and the error message
  rendered verbatim.

## 6. Homelab (`homelab/modules/lib/vibe-kanban-entra.nix`, `hosts/think/think2.nix`)

- New options under `entra`, each emitted as env only when set:
  - `autoReauthIntervalSecs` (nullable int)
  - `atlassian.{site,emailRef,tokenRef,verifyPageId}` (nullable strings)
  - `sgsc.{onboardUrlTemplate, backends}`
- think2's `vibe-kanban-dev.entra` gets: interval 1800; Atlassian site
  `sweetgreen.atlassian.net`, refs `op://Homelab/Jira API/{email,api_token}`,
  page `4796448789`; sgsc template
  `https://claude.sweetgreen.dev/_sgsc/auth/onboard/{backend}`, backends
  `dp,sf`.
- Validate with `nix eval` of the service environment. Open a separate
  homelab PR and merge it after the VK PR, since VK ignores unknown env.

## 7. Verification

- Run `cargo test -p services -- reauth entra_mint cli_tools aws_sso`,
  `cargo test -p mcp`, `cargo test -p server reauth` and
  `cargo clippy --workspace`.
- Run `pnpm run generate-types:check`, `pnpm run check`, `pnpm run lint`,
  the web-core vitest for the card, and `pnpm run format`.
- Run the Codex review loop.
- After merge and deploy, run `curl` against `/api/reauth/targets` on
  think2, then a live `POST /api/reauth/run` for one AWS session, verified
  by the profile probe.

## Risks

- The firecrawl sandbox may lack egress to awsapps, salesforce or the
  gateway. This is detected as a named browser/network error and does not
  hang: the probe text `ERR_` / `chrome-error` fails fast.
- AWS page copy drifts. The classifier matches several button names, and a
  failure transcript records hosts and step names for the next fix.
- TOTP spending in the sweep is bounded by the backoff, and a wrong password
  aborts immediately.
