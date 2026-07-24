# Technical Specification: Microsoft Graph PowerShell (v1.0) CLI capability

## Summary

Add the stable Microsoft Graph PowerShell SDK to the app-managed CLI tool
catalog as `graph-powershell-1.0`. Agents get a version-pinned
`Microsoft.Graph` module tree invoked through a generated wrapper that runs
host PowerShell 7 (`pwsh`). This introduces the catalog's third install
strategy — PowerShell modules — alongside archive binaries and Python venvs.

The `1.0` in the identity is the stable Graph API channel (Microsoft's
production recommendation), not the SDK package major; the SDK publishes 2.x
versions. `Microsoft.Graph.Beta` is not installed.

## Catalog identity

- `CliToolId::GraphPowershell10`, wire id `graph-powershell-1.0` via explicit
  `#[serde(rename)]` (enum-wide kebab-case would yield `graph-powershell10`).
- Display name `Microsoft Graph PowerShell (v1.0)`; docs URL is Microsoft's
  installation guide; pinned version `Microsoft.Graph` **2.38.1** (latest
  stable on PowerShell Gallery, verified 2026-07-24).
- Auth is `Unsupported`: sign-in belongs to the SDK's user-scoped token cache
  (`Connect-MgGraph`, e.g. `-UseDeviceAuthentication`); VK links to vendor
  docs and never provisions tenants, scopes, or secrets.

## Install strategy: `InstallStrategy::PowerShellModule`

1. Requires host `pwsh` (`unsupported_reason` reports "requires PowerShell 7
   (pwsh) on the host" when absent; Windows unsupported like the other
   strategies).
2. `Save-PSResource -Name Microsoft.Graph -Version <pin> -Repository
   PSGallery -Path <staging>/modules -TrustRepository` (PSResourceGet is
   in-box with pwsh 7.4+). Never `Install-Module -Scope CurrentUser`; the
   service user's global module directories are never written.
3. Staged-layout verification: `modules/Microsoft.Graph/<pin>/` must exist
   and the dependency closure must be non-empty (the rollup nupkg alone is
   only a manifest declaring workload modules as dependencies).
4. A generated `graph-powershell-1.0` wrapper (POSIX sh) prepends the
   installed module root to `PSModulePath` and `exec pwsh "$@"`. Prepending
   keeps the pinned SDK winning over user/global copies while `$PSHOME`
   built-ins stay available; `pwsh` resolves from PATH at run time so a host
   copy keeps winning.
5. Atomic promotion via the existing version-directory + `bin/` symlink-last
   model; update and removal reuse the existing flows.
6. `manifest.json` verification string records reality: pinned rollup from
   PSGallery, dependency closure resolved at install time, **no**
   per-artifact hash pinning (weaker than the archive strategy; explicitly
   not claimed).

## Agent invocation

`cli-tools/bin` is already appended to spawned-agent PATH (after host paths):

```bash
graph-powershell-1.0 -NoLogo -NoProfile -Command \
  'Connect-MgGraph -Scopes User.Read.All -UseDeviceAuthentication; Get-MgUser -Top 10'
```

## Renovate

PSGallery serves only a NuGet v2 feed. The cli_tools.rs custom regex manager
gains an optional `registryUrl=` capture; the pin comment routes the `nuget`
datasource to `https://www.powershellgallery.com/api/v2`. A dedicated package
rule keeps these PRs `needs-review` (no sha256 twin; live dependency
resolution).

## Homelab (think2)

`powershell` added to `environment.systemPackages` — immutable `pwsh` for the
VK service and agents via `/run/current-system/sw`. The SDK payload stays
catalog-owned; no Nix activation-script module install. Deployed by comin on
merge; `pwsh --version` as the service user is the post-merge check.

## Out of scope

Beta module, submodule-only installs, in-app device login (follow-up gated on
validating login persistence and `Get-MgContext` invariants), app
registration/secret provisioning, PSGallery mirroring/locking.

## Validation performed

- 14 unit tests in `services::cli_tools` (wire round-trip, catalog
  invariants, entry pinning, wrapper content + real exec/arg-forwarding via a
  stub pwsh).
- Ignored end-to-end test run against real PSGallery with nix-provided pwsh
  7.6.2: exact-version install, workload-module dependency presence,
  `Get-Command Get-MgUser` through the wrapper, exit-code forwarding
  (`exit 42`), user-global module dir untouched, idempotent re-install, clean
  staging, complete removal.
- Generated types, frontend vitest, format, typecheck, lint.

Full SpecKit artifacts: `homelab/specs/vk/4942-add-graph-powers/`
(spec/plan/research/tasks).
