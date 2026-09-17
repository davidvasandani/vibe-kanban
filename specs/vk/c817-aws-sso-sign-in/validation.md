# Validation
Task: vk/c817-aws-sso-sign-in

- `pnpm install --frozen-lockfile`: passed.
- `pnpm run format`: passed.
- Python migration regression suite: 9 tests passed.
- Nix parse and nixfmt checks: passed.
- `nix eval --impure --file tests/vibe-kanban-cluster.nix`: passed, including AWS package and startup dependency assertions.
- Focused worker Rust regression: passed (1 test); the same production function and regression also passed in an isolated rustc harness.
- Frontend type checks (local, remote, web-core, UI) and local/UI ESLint passed in the first run. Full Rust checks are being rerun sequentially after the interrupted run and a missing build-artifact linker failure during concurrent validation. Final results must be recorded before merge.
- Read-only live preflight: coordinator AWS directory exists, owned by its service user with mode 0700; config mode 0600. Worker `.aws` absent, matching the report. No credentials read or logged.
- Live post-deployment CLI/Node SSO acceptance remains pending deployment and availability of a valid SSO session.
