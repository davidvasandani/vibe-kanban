# App-managed PowerShell module CLI tools

Tags: `4942-add-graph-powers`, `cd28-test-powershell`

## When a catalog tool is a module, not a binary

The CLI tools catalog's third install strategy,
`InstallStrategy::PowerShellModule`, covers vendor capabilities shipped as
PowerShell modules (first user: `graph-powershell-1.0`, the stable
Microsoft Graph SDK). The pattern:

- **Host runtime, app payload.** `pwsh` (PowerShell 7) is a detected host
  prerequisite — reported via `unsupported_reason`, never installed by VK.
  On NixOS hosts it comes from `environment.systemPackages`
  (`powershell` in `homelab/hosts/think/think2.nix`). The module tree is
  app-owned and version-pinned.
- **Stage with `Save-PSResource`.** PSResourceGet ships in-box with pwsh
  7.4+. `Save-PSResource -Name <module> -Version <exact> -Repository
  PSGallery -Path <staging> -TrustRepository` downloads the module *and its
  dependency closure* into `<path>/<Module>/<Version>/` layout without
  touching any `PSModulePath` scope. Never use `Install-Module -Scope
  CurrentUser` from the backend: it mutates shared user state, breaks
  atomicity/pinning, and makes removal incomplete.
- **Verify the closure, not just the rollup.** A rollup package's nupkg is
  only a manifest declaring workload modules as dependencies. After saving,
  assert `<module>/<version>/` exists **and** at least one dependency module
  landed beside it, otherwise fail before promotion.
- **Wrapper, not symlinked binary.** Generate a POSIX sh wrapper named like
  the wire id at the version dir's root; `installed_binary_path` points at
  it, so the existing symlink-last promotion, broken-install detection, and
  removal logic apply unchanged. The wrapper *prepends* the app module root
  to `PSModulePath` (pinned SDK wins over user/global copies; `$PSHOME`
  built-ins stay reachable — pwsh always keeps its own module dir) and
  `exec pwsh "$@"`, resolving `pwsh` from PATH at run time so host copies
  keep winning.
- **Honest verification string.** Gallery installs have no per-artifact hash
  pins; the manifest records "dependency closure resolved at install time;
  no per-artifact hash pinning" — same honesty precedent as the az venv
  strategy. Renovate tracks the pin via the `nuget` datasource with
  `registryUrl=https://www.powershellgallery.com/api/v2` (PSGallery is
  v2-only; the v3 index returns 403), always `needs-review`.

## No paths inside generated shell source

Two injection holes surfaced in review, both from interpolating an
app-data-derived path into interpreted source:

- A path inside a single-quoted PowerShell `-Command` string breaks out on an
  embedded quote (think hostile `XDG_DATA_HOME`). Pass dynamic paths to pwsh
  via an **environment variable** (`$env:VK_PSRESOURCE_PATH`) and keep only
  compile-time constants in the command text.
- A path inside a double-quoted POSIX assignment still undergoes `$`/backtick
  expansion every wrapper launch. Single-quote it with `'\''` escaping and
  concatenate the dynamic tail (`'<path>'"${VAR:+:$VAR}"`).

Unit tests pin both behaviours with hostile paths (`o'brien $HOME`).

## Wire-id gotcha

`#[serde(rename_all = "kebab-case")]` renders `GraphPowershell10` as
`graph-powershell10` — a dotted wire id like `graph-powershell-1.0` needs an
explicit `#[serde(rename)]`, and the catalog test must assert the exact wire
string to keep it pinned.

## Token cache boundary (for the future login slice)

Microsoft Graph PowerShell owns authentication via its user-scoped MSAL token
cache under the service user's HOME; `Connect-MgGraph
-UseDeviceAuthentication` persists across processes and `Get-MgContext`
reports context without a Graph call. An in-app login action stays out until
both managed-login invariants from
[cli-tool-oauth-login](cli-tool-oauth-login.md) are validated on the real
host, and tenant/scopes are runtime policy VK must never choose. Production
and development service users must not share a HOME (and therefore a token
cache).

## Live device-code validation

A live `User.Read` device-code test on think2 confirmed that the installed
wrapper loads `Connect-MgGraph` from the pinned `2.38.1` SDK and reaches the
Microsoft device-login flow. It also exposed two operational boundaries:

- A task shell that was already running without `cli-tools/bin` on `PATH` did
  not begin resolving a subsequently installed managed tool. Test the bare
  command first; adding the managed bin directory manually proves the payload,
  but does not prove normal agent `PATH` injection. A newly spawned agent is
  the correct end-to-end check.
- Sweetgreen Entra rejected the otherwise successful primary sign-in with
  Conditional Access error `53003`: the Linux browser reported an unregistered
  device. The wrapper cannot fix tenant policy. Preserve the request ID,
  correlation ID, timestamp, app ID, platform, and device state for the Entra
  administrator, while keeping credentials, device codes, and tokens out of
  task documentation.

When Conditional Access denies the browser flow, the waiting
`Connect-MgGraph` process may surface only a later authentication timeout. The
browser's Entra diagnostic details are therefore the actionable root cause;
the PowerShell timeout is secondary.
