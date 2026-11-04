# Validation: VAS-966

Passed locally:
- `pnpm install --frozen-lockfile`.
- `pnpm run format`, followed by `cargo fmt --all` after review fixes.
- `cargo test -p services github_ --lib`: 26 tests after review fixes.
- `cargo test -p utils github_auth --quiet`: 12 tests, including real git credential refresh and failed-refresh refusal.
- Web-core suite: 108 files, 872 tests; focused Settings suite after adding the health test: 8 tests.
- `pnpm run check`: both frontends, shared packages, all backend workspaces including remote.
- Frontend lint in local-web, web-core, remote-web and ui; `pnpm run backend:lint` for all targets with warnings denied, including remote; unused-i18n check.
- Generated TypeScript and schemas through `pnpm run generate-types`, then `target/debug/generate_types --check`.
- Three independent Codex review rounds; final round found no actionable regressions. See `review.md`.

The initial concurrent Cargo builds hit vendored OpenSSL/missing-crate artifact errors. Serial builds completed successfully. Generated TypeScript retains the generator's existing whitespace style; it was not hand-edited.

Live sweetgreen `get_pr`/`list_pr_checks`/push/create/merge acceptance is not verified. The App ID, installation ID and private-key reference were requested but have not been provided. No installation, credentials or repository rulesets were changed. Admin bypass remains deferred.
