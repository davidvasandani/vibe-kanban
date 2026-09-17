# Validation
Task: vk/c817-aws-sso-sign-in

- `pnpm install --frozen-lockfile`: passed.
- `pnpm run format`: passed.
- Python migration regression suite: 9 tests passed.
- Nix parse and nixfmt checks: passed.
- `nix eval --impure --file tests/vibe-kanban-cluster.nix`: passed, including AWS package and startup dependency assertions.
- Focused worker Rust regression: passed (1 test); the same production function and regression also passed in an isolated rustc harness.
- Frontend type checks (local, remote, web-core, UI) and local/UI ESLint passed locally. Broad local Rust check/lint attempts were interrupted or hit a missing build artifact; do not count them as passing.
- GitHub application CI run 35197496151 passed all six checks: changes, frontend-checks, backend-schema-checks, backend-clippy, backend-test, remote-test. This supplies the full repository verification that was interrupted locally.
- Read-only live preflight: coordinator AWS directory exists, owned by its service user with mode 0700; config mode 0600. Worker `.aws` absent, matching the report. No credentials read or logged.
- Deployment PR https://github.com/davidvasandani/homelab/pull/1240 and application PR https://github.com/davidvasandani/vibe-kanban/pull/302 merged. Shared AWS-state setup is active on think2 and think5. An actual agent shell on think3 sees all 31 ai-foundry profiles.
- Live non-login agent shell resolves AWS CLI 2.34.24 from PATH. Both `aws sts get-caller-identity` and Node `@aws-sdk/credential-provider-node` 3.972.83 `defaultProvider()` reach the intended SSO state but report an expired token. Removed static access-key variables for both checks; no credential values were printed. Successful live authentication remains unverified until a fresh Settings sign-in.
- Live login-shell checking exposed a second PATH boundary: shell startup replaces the service PATH. Follow-up deployment PR https://github.com/davidvasandani/homelab/pull/1242 adds AWS CLI to the system profile too. Its Nix evaluation, formatting, 9 Python tests and independent Codex review passed.
