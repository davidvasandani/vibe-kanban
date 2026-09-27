# SPEC: Unattended re-authentication ("auto auth all the things")

Task: `vk/ba8f-auth-auth`

## Problem

The operator keeps a macOS skill (`aws-sso-reauth`) that re-authenticates
every CLI and gateway credential an agent needs with no human in the loop: it
drives a throwaway Chrome through the Entra sign-in (email → password →
authenticator TOTP, all read from 1Password), then clicks through each
provider's approval. It has five engines:

| Engine | What it refreshes | Mechanism on the Mac |
| --- | --- | --- |
| `aws` | AWS IAM Identity Center (SSO) token | `aws sso login --no-browser` device code → Chrome → Entra → *Confirm and continue* → *Allow* |
| `az` | Azure CLI MSAL cache | Tier 1 silent `get-access-token`; Tier 2 device-code + Chrome |
| `sgsc` | sgsc-mcp gateway per-user backend OAuth (`dp`, `sf`, …) | open `/_sgsc/auth/onboard/<backend>` → Entra → provider hop (Snowflake *ENTRA_ID_SSO* button; Salesforce through the MyApps tile) → "Connected" |
| `snow` | Snowflake CLI externalbrowser token | `BROWSER` hook captures the SSO URL; local Chrome completes the loopback redirect |
| `acli` | `acli confluence` | API token from 1Password piped to `acli confluence auth login --token`, verified by a real page read |

Vibe Kanban (VK) runs agents on a NixOS cluster (coordinator think2 plus
workers). VK already has **part** of this:

- `crates/services/src/services/entra_mint.rs` drives the self-hosted
  firecrawl browser service over its `/v2/interact` API, using a **persistent
  browser profile** (`vk-entra`) and 1Password **Connect** for the password and
  TOTP. It mints `az` and Graph PowerShell tokens and supplies a local
  Chromium for `mgc-beta`'s own loopback login. The deployment
  (`homelab/modules/lib/vibe-kanban-entra.nix`) wires all of its settings.
- Settings → AWS manages SSO profiles and signs them in with
  `aws sso login --use-device-code` in a PTY, but a human must open the URL,
  sign in to Entra, and click *Allow*.
- `acli` is a managed CLI tool whose authentication is `Unsupported`.
- The sgsc-mcp gateway is a settings-owned OAuth MCP. When one of its backends
  needs onboarding, a tool returns `reauth_required` with an onboard URL that
  only a human can finish.

Every expiry therefore still stops an agent until the operator signs in by
hand, and nothing re-authenticates proactively.

## Goal

VK re-authenticates **every credential it can verify** with no human
interaction, both **on demand** (an agent or the Settings UI asks for it) and
**automatically** (a background sweep that finds expired credentials and
repairs them), reusing the Entra/1Password/browser machinery that already
ships.

## Adaptation decisions (Mac → VK)

1. **Browser**: no throwaway local Chrome. The firecrawl browser service with
   the persistent `vk-entra` profile is already the deployment's browser.
   Because the profile keeps the Entra session cookie, most runs are silent
   and spend no password or TOTP; interactive sign-in is the fallback. The
   Little Snitch precheck has no equivalent and is dropped. A network-layer
   failure is still reported as a named error.
2. **1Password**: no SDK service-account token or Keychain. Use the
   1Password Connect credentials `entra_mint` already uses. Add `op://`
   reference resolution through Connect (vault by name, item by title, field
   by label or id) so non-Entra secrets (the Atlassian token) can be named by
   reference, as on the Mac.
3. **Config isolation**: the Mac skill uses private `AWS_CONFIG_FILE` and
   `SNOWFLAKE_HOME`. VK already owns a managed AWS config (Settings → AWS) and
   the shared cluster AWS state, so AWS re-auth acts on the profiles/sessions
   VK manages. It never writes profile config.
4. **Identity constants** come from the deployment environment
   (`VK_ENTRA_*`), not from source.
5. **Scope of engines in VK**:
   - `aws`: **new**. Automates the existing device-code login. VK runs
     `aws sso login --sso-session <s> | --profile <p> --use-device-code
     --no-browser`, scrapes the verification URL, drives it in the firecrawl
     browser through Entra and the AWS approval pages, and succeeds only if
     the independent profile probe then reports authenticated.
   - `az`, `graph-powershell`, `mgc-beta`: **existing** Entra flows, now also
     reachable through the new unattended entry points and the sweep.
   - `acli`: **new**. Confluence API-token login from 1Password, verified by
     a real content read (never `auth status`).
   - `sgsc`: **new**. Drives the gateway onboard URL for named backends in the
     firecrawl browser. On demand only: the gateway has no non-mutating
     status probe, so a sweep could not tell expired from healthy.
   - `snow`: **out of scope**. The Snowflake CLI is neither installed nor
     managed by VK. Agents reach Snowflake through the sgsc `dp` backend,
     which this task covers. `entra_mint::native_browser_login` already has
     the shape `snow` would need if it is added to the CLI catalog later.

## Functional requirements

- **FR-1 Target registry.** A server-side registry lists re-auth targets with
  a stable id (`aws-sso:<session>`, `aws-profile:<profile>`,
  `cli-tool:<id>`, `sgsc:<backend>`), a kind, a display label, whether it is
  swept automatically, its last run (time, outcome, message), and whether it
  is configured. The browser/agent never supplies commands, URLs or secrets:
  only a target id, validated server-side.
- **FR-2 Unattended AWS SSO.** For a VK-managed SSO session or profile, run
  the device-code login without a PTY. Capture the verification URL, drive it
  in the firecrawl browser (Entra email/password/TOTP when needed, *Stay
  signed in*, AWS *Confirm and continue* / *Allow access*), wait for the CLI
  to exit, then verify with the existing profile probe. Retry up to three
  times on an Entra flow timeout or rejected code, with a fresh device code
  each time. One process per lock key, sharing the existing AWS login lock.
- **FR-3 Unattended acli Confluence.** Resolve the Atlassian site, email and
  API token (1Password references from the environment), run
  `acli confluence auth login --site --email --token` with the token and a
  trailing newline on stdin, then verify with a Confluence read. The token
  never appears in argv, logs or responses.
- **FR-4 sgsc backend onboarding.** For each requested backend, open the
  configured onboard URL template in the firecrawl browser, complete Entra,
  perform the provider hop (a Snowflake *ENTRA_ID_SSO* button; for
  Salesforce, the MyApps tile and then the onboard URL again), and succeed
  only when the URL reaches `/_sgsc/auth/callback/<backend>` or the page says
  "Connected". Backend names are validated (`[a-z0-9_-]{1,32}`).
- **FR-5 Shared Entra driver.** The Entra step machine in `entra_mint`
  (email, password, TOTP, stay-signed-in, account picker) is generalized to
  run until a caller-supplied completion predicate holds. The OAuth mint,
  AWS, and sgsc flows all use it. Wrong-password copy aborts immediately
  instead of retrying, to protect the account from lockout.
- **FR-6 On-demand API.** `GET /api/reauth/targets` returns the registry with
  status. `POST /api/reauth/run` `{target}` runs one target to completion
  (bounded at 6 minutes) and returns the outcome and a progress transcript.
  A request for a target that is already running joins that run
  (`already_running: true`) and does not start a second browser flow.
  `GET /api/reauth/runs` returns only the last runs, without probing, so a
  UI can poll it cheaply.
- **FR-7 Agent tool.** The VK MCP server exposes `reauthenticate` (target id,
  optional) and `list_reauth_targets`. Agents call these when a command fails
  with an expired-credential error, the same trigger the Mac skill uses.
  Without a target, the tool re-authenticates every swept target that is
  currently unauthenticated.
- **FR-8 Automatic sweep.** Opt-in with `VK_AUTO_REAUTH_INTERVAL_SECS` (unset
  or 0 means off; the minimum is 300). Each tick probes the swept targets
  (AWS sessions that have unauthenticated profiles; CLI tools with Entra or
  1Password login that report unauthenticated) and re-authenticates them one
  at a time. A target that fails backs off exponentially (up to 6 hours)
  before its next automatic attempt, so a broken credential cannot
  repeatedly spend TOTP codes or lock the account. The first tick is delayed
  after startup.
- **FR-8a Refusal gate and escalation.** A definitive refusal marks the
  target refused.
  - Agent and sweep runs of a refused target return the recorded refusal
    without starting a flow. Only a manual run from Settings clears it.
  - A refusal, or a third consecutive automatic failure, logs one
    `vk-reauth: operator action required` line. The homelab
    `vibe-kanban-reauth-alert` unit pages ntfy on that line.
- **FR-8b One profile writer.** Every firecrawl session on the persistent
  Entra profile holds a process-wide lock, so concurrent sign-ins queue
  instead of racing profile saves.
- **FR-9 Settings UI.** The CLI Tools settings page shows an *Unattended
  re-authentication* card that lists targets with their last outcome, a
  per-target *Re-authenticate* button, and *Re-authenticate all*. It uses the
  existing machine-aware settings client.
- **FR-10 Not configured is explicit.** Without the Entra/Connect/browser
  environment, or without the Atlassian or sgsc settings, the affected
  targets report `not_configured` with the missing variable name. They never
  fail silently.
- **FR-11 Secret hygiene.** Passwords, TOTP codes, API tokens, device codes
  and authenticated URLs (query strings) never appear in logs, API
  responses, transcripts or process argv. Transcripts may name steps and URL
  hosts and paths.

## Non-goals

- Snowflake CLI (`snow`) support (see decision 5).
- Replacing the interactive PTY sign-ins. They remain, unchanged, as the
  manual path.
- Writing AWS profile config, or creating SSO sessions and profiles.
- Storing any credential in VK. Tokens stay in each vendor's own store.

## Deployment (homelab `modules/vibe-kanban-rebuild.nix` and friends)

- Coordinator/dev service environment: `VK_AUTO_REAUTH_INTERVAL_SECS`,
  `VK_ATLASSIAN_SITE`, `VK_ATLASSIAN_EMAIL_REF`, `VK_ATLASSIAN_TOKEN_REF`,
  `VK_SGSC_ONBOARD_URL_TEMPLATE`, exposed as options beside the existing
  `entra` options (in `lib/vibe-kanban-entra.nix`) and set for think2.
- No new secret material in the unit. 1Password references are not secrets,
  and Connect credentials are already provisioned.

## Acceptance criteria

- [ ] `cargo test -p services reauth entra_mint cli_tools aws_sso` covers:
      device-URL scraping; the AWS approval-page classifier; the sgsc
      success detector; `op://` reference parsing and Connect field lookup;
      target-id parsing and validation; sweep backoff scheduling;
      transcript redaction (no token, code, or query string); and the
      conflict guard.
- [ ] `cargo test -p mcp` confirms the new tools are registered in the
      correct modes.
- [ ] Frontend: a component test for the card's list and run states;
      `pnpm run check` and `pnpm run lint` pass.
- [ ] `pnpm run generate-types:check` and `pnpm run format` are clean.
- [ ] The homelab module evaluates (`nix eval` of think2's service
      environment includes the new variables).
- [ ] After deployment, `GET /api/reauth/targets` on think2 lists the AWS
      sessions and the Entra tools with real status. A live run of at least
      one target returns `succeeded`, verified by its independent probe.
