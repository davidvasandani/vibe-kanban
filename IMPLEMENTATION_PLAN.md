# Implementation Plan: Microsoft Graph PowerShell (v1.0) CLI capability

1. Pin `Microsoft.Graph` 2.38.1 (latest stable on PSGallery, verified) as
   `MICROSOFT_GRAPH_PS_VERSION` in
   `crates/services/src/services/cli_tools.rs`, with a Renovate marker using
   the `nuget` datasource and an explicit
   `registryUrl=https://www.powershellgallery.com/api/v2` (PSGallery has no
   v3 feed).
2. Add `CliToolId::GraphPowershell10` with an explicit
   `#[serde(rename = "graph-powershell-1.0")]` (kebab-case autorename would
   drop the dot), extend `ALL`, `dir_name()`, and add the catalog entry:
   display name `Microsoft Graph PowerShell (v1.0)`, Microsoft installation
   guide as docs URL, empty `sources`, auth `Unsupported` (externally managed
   token cache).
3. Add `InstallStrategy::PowerShellModule { module }`:
   - `unsupported_reason`: Windows unsupported; requires host `pwsh`.
   - `install_powershell_module`: stage via `Save-PSResource` (exact
     version, PSGallery, `-TrustRepository`, `-Path <stage>/modules`),
     verify `modules/<module>/<version>/` exists and the dependency closure
     is non-empty (rollup nupkg alone is only a manifest), generate the
     `graph-powershell-1.0` wrapper (prepend final module root to
     `PSModulePath`, `exec pwsh "$@"`, mode 0755), promote atomically with
     `promote_staged_version`, return an honest verification string (no
     artifact-hash claim).
   - `installed_binary_path`: wrapper at the version dir root, so the
     existing symlink/manifest/remove logic applies unchanged.
4. Tests: catalog-entry pinning test (mirroring acli's), wrapper content
   tests (prepend-not-clobber, exec + `"$@"`), a stub-pwsh execution test for
   argument/env forwarding, extend the login-eligibility invariant test, and
   an `#[ignore]`d end-to-end PSGallery install/remove test (exact version,
   `Get-Command Get-MgUser` via the wrapper, exit-code forwarding, untouched
   user-global module dir, idempotent re-install, clean staging, full
   removal).
5. Regenerate `shared/types.ts` (`pnpm run generate-types`); no
   `generate_types.rs` change needed (`CliToolId` already declared).
6. Frontend: typed `graph-powershell-1.0` fixture in
   `packages/web-core/src/shared/lib/cliToolLogin.test.ts` asserting login is
   never offered; the settings section itself is data-driven and needs no
   code change.
7. `renovate.json`: optional `registryUrl=` capture in the cli_tools custom
   manager; new `nuget` package rule — never auto-merge, `needs-review`,
   PR note explaining there is no sha256 twin and dependencies resolve at
   install time.
8. Docs: new `docs/settings/cli-tools.mdx` (catalogue overview + Graph
   PowerShell section: stable v1.0 vs beta, wrapper invocation, externally
   managed authentication, service-user credential boundary), wired into
   `docs/docs.json` navigation.
9. Homelab: add `powershell` to `environment.systemPackages` in
   `homelab/hosts/think/think2.nix` (immutable pwsh for service + agents;
   SDK payload stays catalog-owned). Post-merge check: `pwsh --version` as
   the service user.
10. Validate: services unit tests, real e2e install with nix-provided pwsh,
    `generate-types:check`, web-core vitest, `pnpm run format`, `pnpm run
    check`, `pnpm run lint`; then independent Codex review and knowledge-base
    distillation.

SpecKit artifacts: `homelab/specs/vk/4942-add-graph-powers/`.
