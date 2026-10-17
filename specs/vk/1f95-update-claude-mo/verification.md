# Verification: Claude Fable 5.1 and Claude Code 2.1.268

## Passed checks

- `pnpm install --frozen-lockfile`
- `cargo test -p executors executors::claude::tests -- --nocapture`
  - 41 passed, 0 failed
- `pnpm run generate-types:check`
  - 10 schemas generated for comparison
  - `shared/types.ts` up to date
- `pnpm run format`
- `cargo check -p executors`
- `git diff --check`
- `nix eval --raw .#nixosConfigurations.think5.pkgs.nodejs.version`
  - result: `24.15.0`

After review-driven context-window fixes, the full 41-test Claude suite,
`cargo check -p executors`, `cargo fmt --all`, and `git diff --check` passed
again. The final independent Codex review reported no significant findings.

## External artifact evidence

- npm `latest`: `@anthropic-ai/claude-code@2.1.268`
- npm `next`: `2.1.269` (not selected)
- Native artifact inspected:
  `@anthropic-ai/claude-code-linux-x64@2.1.268`
- Native catalog confirmed `claude-fable-5-1`, five effort levels, existing
  family aliases, and the exact safety canonical/alias mappings recorded in
  `research.md`.

## Environment note

An initial attempt to run two Cargo test commands concurrently raced on their
shared target directory and failed while creating an archive. The commands were
rerun serially; the complete Claude test module passed. A first serial build hit
the initial 120-second wall-clock timeout while compiling dependencies, then
completed successfully with an extended timeout.
