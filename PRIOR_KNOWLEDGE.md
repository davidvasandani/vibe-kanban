# Prior knowledge: unattended re-authentication (`vk/ba8f-auth-auth`)

I searched both knowledge bases (`wiki/` and `docs/knowledge-base/`) for Entra,
1Password, AWS SSO, CLI login, MCP OAuth, browser and deployment topics. The
matches below are what the spec and plan build on.

## Reuse, don't rebuild

- **Entra minting through the firecrawl browser already exists**
  (`crates/services/src/services/entra_mint.rs`; there is no wiki page yet).
  - It drives the self-hosted firecrawl `/v2/interact` API. `mint()` runs a
    silent `prompt=none` attempt against the saved `vk-entra` browser profile,
    then falls back to an interactive step machine (email, password, TOTP,
    stay-signed-in, account picker). The password and TOTP come from
    1Password **Connect**, which computes the TOTP server-side.
  - Step detection keys on page *text*. Entra keeps both the `loginfmt` and
    `passwd` inputs visible in the DOM, so checking for elements alone
    re-submits the email forever.
  - `native_browser_login` covers tools that run their own loopback OAuth. It
    installs an `xdg-open` shim to capture the URL and runs a local headless
    Chromium seeded with the profile's cookies.
  - The deployment wires every setting through `VK_ENTRA_*` env vars
    (`homelab/modules/lib/vibe-kanban-entra.nix`). They are enabled on think2
    (`services.vibe-kanban-dev.entra`).
- **The managed CLI catalog** (`wiki/managed-cli-tool-catalog.md`,
  `docs/knowledge-base/cli-tool-oauth-login.md`):
  - Login strategies are `Command` (PTY), `EntraMint`, `EntraNativeBrowser` and
    `Unsupported`.
  - Rules: commands stay compiled into the server; resolve the effective
    binary the way agents do (host copy first); keep one login per tool; and a
    zero exit is not authentication, so an independent probe must confirm it.
  - `acli` is currently `Unsupported`.
- **AWS SSO** (`docs/knowledge-base/aws-sso-profile-management.md`,
  `wiki/aws-sso-agent-state.md`):
  - VK guest-edits `~/.aws/config`; tokens stay in `~/.aws/sso/cache`.
  - Login is session-first and always uses `--use-device-code`, because VK is
    headless.
  - The login lock key is the profile's `sso_session`, so one token serves
    every profile in that session.
  - Status probes share a process-wide budget of four concurrent probes, with
    lazy admission. Completed status checks and successful authentication are
    different assertions.
  - `.aws` is shared across the cluster through NFS. A running turn that
    predates the link may not see it.

## Constraints and gotchas to carry forward

- **Conditional Access**: Sweetgreen Entra refuses Microsoft's *own*
  device-code flow (`AADSTS53003`) even from a compliant device
  (`docs/knowledge-base/powershell-module-cli-tools.md`). AWS SSO's device
  grant is AWS's, and its Entra hop is a SAML browser sign-in, which the Mac
  skill completes routinely. Preserve Entra request/correlation IDs in
  diagnostics when CA denies a flow.
- **Secrets** (constitution XIII and XVII; `workspace-environment-inheritance`):
  never put tokens, codes or authenticated URLs in argv, logs or API
  responses. Pass secrets to child processes over stdin.
- **MCP OAuth** (`wiki/mcp-oauth-connection-identity.md`, `mcp-oauth-connect`):
  - The sgsc-mcp gateway connection itself is VK-managed, and it refreshes on
    a 401/403 in `mcp_gateway`.
  - Backend onboarding (`/_sgsc/auth/onboard/<backend>`) is a separate
    per-user grant that VK does not model. VK has no `reauth_required`
    concept anywhere.
- **Deployment** (`wiki/self-hosted-deployment.md`): services exec immutable
  releases. A merge to `main` deploys through the reconciler, so live
  validation happens after the merge.
- **Background loops** belong in the deployment setup
  (`crates/local-deployment/src/lib.rs`, where `start_cleanup_tasks` and
  `PrMonitorService::spawn` start).

## Knowledge-base gaps this task should fill

- There is no page on the Entra/firecrawl minting architecture. After
  shipping, record the generalized unattended re-auth design, its engines and
  their gotchas.
