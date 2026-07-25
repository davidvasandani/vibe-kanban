# Prior Knowledge: Microsoft Graph PowerShell (v1.0) CLI capability

The project knowledge base is populated. The most relevant pages are
`docs/knowledge-base/cli-tool-oauth-login.md` and, secondarily,
`docs/knowledge-base/workspace-environment-inheritance.md` and
`docs/knowledge-base/aws-sso-profile-management.md`.

## Managed CLI tool login boundary (`cli-tool-oauth-login`)

- VK orchestrates vendor CLIs; it must not become an OAuth client or
  credential store. Tokens live in the CLI's normal host-side storage.
- In-app login is offered only when two invariants both hold: (1) credentials
  survive the login child process and are usable by later agents, and (2) a
  separate non-secret command independently verifies authentication. The
  existing catalog already rules out the pinned Graph **beta** CLI
  (`mgc-beta`) on invariant 2 — so shipping `graph-powershell-1.0` with
  `CliToolAuthStrategy::Unsupported` and vendor-doc links matches
  established policy. `Connect-MgGraph -UseDeviceAuthentication` +
  `Get-MgContext` may satisfy both invariants later, but that is a follow-up
  acceptance test, not an assumption.
- Login commands are compiled into the server catalog; nothing command-like
  is accepted from the browser. Auth probes run with a short timeout,
  `kill_on_drop`, and a minimal allowlisted environment.

## CLI tools catalog mechanics (from the catalog source itself)

- One app-owned directory (`assets::cli_tools_dir()`); only `cli-tools/bin`
  is exposed on spawned-agent PATH, appended after host paths so
  host-provided copies win.
- Installs are atomic: staging → version-directory rename → `bin/` symlink
  swapped last. Removal deletes the symlink then the tool dir. A manifest
  without a working symlink reads as "not installed" so the UI offers a
  clean re-install.
- The venv strategy (az) already established the precedent for
  weaker-than-archive verification being recorded honestly in the manifest
  verification string — the PowerShell module strategy follows it.
- Renovate custom regex manager tracks `// renovate: datasource=... depName=...`
  comments above `*_VERSION` consts; catalog pins are never auto-merged.

## Environment boundaries (`workspace-environment-inheritance`)

- Agent PATH assembly happens in `crates/local-deployment/src/container.rs`
  (merge of inherited PATH + `cli_tools_bin_dir()`); a new tool needs no
  wiring there — landing a wrapper in `cli-tools/bin` is sufficient.
- Never mutate the long-lived server environment or write env files as a
  side channel; the wrapper script carries its own `PSModulePath` setup.

## Host provisioning precedent (`aws-sso-profile-management`, think2.nix)

- Host runtimes for agent tooling (e.g. `gws`, `claude-code`) are provided
  by think2's `environment.systemPackages` and reach the service via
  `/run/current-system/sw`; the same route serves `pwsh`. App-managed
  payloads stay out of Nix.
