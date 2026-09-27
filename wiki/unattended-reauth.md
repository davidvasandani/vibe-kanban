# Unattended re-authentication

`crates/services/src/services/reauth/` re-authenticates expired credentials
without a human. It covers AWS SSO scopes, the Entra CLIs (`az`, Graph
PowerShell, `mgc-beta`), `acli confluence` and sgsc-mcp gateway backends.
It adapts the operator's macOS `aws-sso-reauth` skill. Instead of a
throwaway Chrome and a 1Password service-account token, it reuses what
`entra_mint` already ships: the firecrawl browser with the persistent
`vk-entra` profile, and 1Password Connect for the password and TOTP.

## Shape

- **Targets are server-defined ids.** They are `aws-sso:<session>`,
  `aws-profile:<profile>`, `cli-tool:<id>` and `sgsc:<backend>`.
  - Callers name an id and nothing else.
  - Only ids the host *lists* may start (`canonicalize`). A run nobody can
    see can't be polled, and if it is refused it can't be reset from
    Settings.
  - AWS profiles resolve to the one scope id that discovery lists. Legacy
    aliases of a start URL would otherwise get separate registry entries and
    separate refusal gates for the same token.
- **Runs are detached.** A run is owned by a spawned task and outlives the
  request.
  - `POST /api/reauth/run` waits at most 55 s. The MCP tool uses 50 s,
    because Codex's tool deadline is 60 s.
  - Polling goes through `GET /api/reauth/runs` / `list_reauth_runs`, which
    read the registry only. Never poll `list_reauth_targets`: it re-probes
    everything and can overrun a tool deadline.
- **`entra_mint::drive`** is the generic step loop.
  - Each tick it offers the page to the caller's verdict first, then
    handles Entra steps itself.
  - Refusals, flow timeouts, rejected codes and network failures come back
    as typed `EntraError`s. `should_retry` retries only `FlowTimeout` and
    `BadCode`, and at most 3 times.
- **A batch runs sequentially in one task, claimed up front.** Every
  browser engine needs the single profile. Starting them together only
  parks later ones on the profile lock, and an AWS device code expires
  while it waits.

## Lockout safety (the hard part)

One shared Entra account and one 1Password password back every
browser-driven target. So:

- **A refusal is about the account, not the target.** A definitive refusal
  (wrong password, AADSTS53003 Conditional Access, locked account) latches
  process-wide.
  - Agent and sweep runs of any Entra-backed target then settle as
    `refused` without signing in.
  - A manual run may retry a refusal recorded *before* it was requested:
    that is the operator reset.
  - A refusal hit *during* a manual "re-authenticate all" still stops the
    rest of the batch. Otherwise one click resubmits the stale password
    once per target.
- **Check, attempt and latch as one step.** A second lock
  (`entra_run_lock`) serialises Entra-backed runs from the refusal check
  through the latch update. The profile lock alone lets two runs both pass
  the check.
- **Refusals arrive on every path**: page copy, the OAuth redirect's
  `error_description`, the token endpoint, and tool output.
  `normalize_refusal` classifies all of them. Missing one path silently
  disables the gate.
- **Credentials go only to HTTPS Entra origins.** Page text alone must never
  decide where the password goes. The driver visits AWS, gateway and
  provider pages, and any of them may say "enter password".

## Probes: evidence, not exit codes

- A CLI probe exits nonzero for an outage, a lost permission or a deleted
  verification page just as it does for an expiry. Re-auth counts a tool as
  expired only when the failed probe's output shows an auth error
  (`auth_failure_confirmed`, per tool). Anything else is `unknown` and is
  never repaired automatically. The CLI Tools card keeps its exit-code
  contract.
- `az account show` reads cached subscriptions and stays green after the
  refresh token dies. Probe with `az account get-access-token --output none`.
- Graph PowerShell connects in a vk-owned profile block. That block used to
  swallow refresh failures with `Write-Verbose`, which made an outage look
  like "never signed in". It now uses `Write-Warning`, and
  `migrate_graph_powershell_profile` rewrites old blocks before the probe is
  trusted.
- For AWS, one successful STS call does not prove the scope is healthy: the
  CLI can answer from cached role credentials after the SSO token expired.
  Discovery therefore probes every member, through `list_profile_statuses`
  (its bounded, lazily admitted stream). A plain `join_all` starts every
  admission timeout at once, and later scopes time out unprobed.
- AWS verification after a login passes if *any* of up to five members
  authenticates, so a profile with a removed role can't fail a repair that
  worked. It runs only after the CLI exited 0 with a fresh token.
- **Only listing gets a deadline.** Listing gives AWS probing 40 s and then
  shows the scopes as unprobed. Repair paths wait for real results, or a
  timeout would hide exactly the expired scope.
- An sgsc backend has no status probe, so it is on demand only and never
  swept. Success must be the gateway affirmatively saying "connected" for
  *this* backend. Reject:
  - `?error=` callbacks;
  - "not connected" and "disconnected";
  - another backend's success page.

## Browser gotchas

- **Never echo a failed script's response.** The firecrawl `execute`
  response carries Playwright call logs, which can include the value passed
  to `fill`. `script_failure_message` keeps only a coarse cause.
- **Close a dropped session before releasing the profile.** `Drop` for
  `BrowserSession` closes the remote session in the background, holding the
  profile lock until that is done. Otherwise an abandoned `saveChanges`
  session (for example from a timed-out run) saves over the next writer's
  cookies.
- **Bound every run** (`ENGINE_TIMEOUT`, 20 min). Dropping the engine kills
  children through `kill_on_drop`, and `LoginChild` cancels its watcher on
  drop. A hung `apply_az` would otherwise hold the Entra run lock forever.
- **Complete provider hops in the driven page.** The Salesforce MyApps tile
  is opened in the same tab (its link's `target` removed), so the
  credential-capable driver sees and completes any Entra challenge. A popup
  that nobody drives just times out.
- **`mgc-beta`'s local browser only reuses cookies.** It can't enter
  factors, so `refresh_session` mints a throwaway Graph token through
  `drive` first.
- AWS approval pages can render on `*.awsapps.com`,
  `device.sso.<region>.amazonaws.com` or regional `<region>.signin.aws`.

## Settings card

- Apply a response only while its host is still selected. A late answer
  from the previous host would repaint the card, and its buttons would then
  act on the wrong machine.
- Merge runs by identity and progress (`isNewerRun`). A later `started_at`
  wins. The same run may only move forward: to a terminal outcome, or to a
  longer transcript. A full refresh keeps any newer run the card already
  holds.
- If a bulk request stops waiting on discovery, keep polling for a bounded
  window. The server keeps going and starts repairs on its own.

## Deployment

The homelab `entra` options (`homelab/modules/lib/vibe-kanban-entra.nix`)
emit `VK_AUTO_REAUTH_INTERVAL_SECS`, `VK_ATLASSIAN_*` and `VK_SGSC_*`, all
non-secret. `vibe-kanban-reauth-alert` follows the journal for the pinned
`vk-reauth: operator action required` line and pages ntfy. Nothing
`Requires=` it.

## Review note

It took 23 independent Codex review passes to converge. Nearly every
finding was one of the classes above, so check new engines against this
page.

## Contributed by

- vk/ba8f-auth-auth
