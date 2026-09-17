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

## Authentication-probe follow-up evidence

- After the user's new Settings sign-in, the actual agent CLI STS check passed in 1.54 seconds and Node's default provider resolved temporary credentials successfully; no secrets printed.
- Coordinator single STS probe: success, 1.23 seconds. Simultaneous 31-profile reproduction with the old five-second budget: 30 timed out, one succeeded.
- Coordinator reproduction with four concurrent probes and 15-second execution budgets: all 31 succeeded in 15.89 seconds; slowest probe 3.1 seconds.
- Required dependency setup and repository formatting passed. `cargo test -p services aws_sso` passed all 34 tests, including the five new deterministic concurrency/budget/cancellation regressions. Deployed API validation follows merge/rollout.
- Actual pre-fix `/api/aws/profiles`: all 31 statuses unknown, 10.44 seconds.
- Reproduction including the application's additional AWS CLI version lookup before each STS call: four shared slots, 30-second admission budget, all 31 succeeded in 21.82 seconds.

- Follow-up PR https://github.com/davidvasandani/vibe-kanban/pull/304 merged as `d154bab586f257dd8568aa250460b723e606a6b7`. Implementation CI run 35205139451 passed backend tests, Clippy, generated-type/SQLx checks, and remote tests; frontend checks were correctly skipped for the backend-only change.

## Deployed follow-up check (2026-09-17 11:12–11:15 UTC)

- Coordinator `/api/info` reports `d154bab`, matching the merged probe fix. Rebuild service finished.
- First deployed refresh: 2 authenticated, 29 unknown. A subsequent refresh reported 8 authenticated, 19 busy-admission results, and 4 execution timeouts.
- Stopped the rollout poller, allowed outstanding requests to finish, and ran one isolated refresh: 4 authenticated, 27 unknown. **All-31 deployed acceptance did not pass.**
- Host has six CPUs and load average around 66. Vibe Kanban has a three-CPU quota; its cgroup CPU pressure was about 78% some / 39% full over ten seconds. Its server process had 19 Git children during the check. These observations show substantial competing work; they do not establish the individual Git operations' purpose or justify changing another service.
- Original host-to-agent acceptance passed after fresh sign-in (CLI STS and Node default provider). The concurrency regression suite and independent code review passed. The probe fix is deployed, but remaining timeouts under sustained production CPU contention remain an unresolved operational limitation. No all-authenticated rollout marker was produced.

## Lazy-admission follow-up

- Fixed an additional cause of busy results: all 31 eagerly polled profile futures started admission deadlines together. Each refresh now admits at most four futures into the global semaphore and resolves the executable once per refresh.
- `pnpm install --frozen-lockfile`, `pnpm run format`, and `cargo test -p services aws_sso` passed (34 AWS tests). The overlapping 31-profile regression now simulates ten-second probes, runs beyond the 30-second admission window overall, and verifies every identity in order with peak concurrency four.
- Independent Codex CLI review found no actionable regressions. Deployment verification remains pending for this follow-up; #304's partial live result is not sufficient.

- Final follow-up verification: all 35 AWS tests passed, including a compile-time assertion that the profile-list future is Send for the Axum handler. CI run 35227546136 passed backend tests, Clippy, generated types/SQLx, and remote tests. Final independent Codex review found no actionable regressions. PR #306 merged as `6cb1d719f72bcd47b4428e4e9382e133d5f9145b`.

## Deployed lazy-admission check (2026-09-17 17:25 UTC)

- Coordinator `/api/info` reports `6cb1d71`, matching PR #306. A single `/api/aws/profiles` refresh returned all 31 profiles unauthenticated, with **zero unknown/busy/timeout results**.
- An actual agent `aws sts get-caller-identity` call, with ambient static-key variables removed, confirmed an expired SSO session. No credential values or identity details were logged.
- The deployment crossed the previous credential lifetime. Fresh Settings sign-in is required for the final all-authenticated check; the observed expired-session result must not be described as successful authentication.
- Stopped the rollout monitor; no repeating AWS verification requests remain.
