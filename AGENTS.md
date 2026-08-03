# Repository Guidelines

## Project Structure & Module Organization
- `crates/`: Rust workspace crates — `server` (API + bins), `db` (SQLx models/migrations), `executors`, `services`, `utils`, `git` (Git operations), `api-types` (shared API types for local + remote), `review` (PR review tool), `deployment`, `local-deployment`, `remote`.
- `packages/local-web/`: Local React + TypeScript app entrypoint (Vite, Tailwind). Shell source in `packages/local-web/src`.
- `packages/remote-web/`: Remote deployment frontend entrypoint.
- `packages/web-core/`: Shared React + TypeScript frontend library used by local + remote web (`packages/web-core/src`).
- `shared/`: Generated TypeScript types (`shared/types.ts`, `shared/remote-types.ts`) and agent tool schemas (`shared/schemas/`). Do not edit generated files directly.
- `assets/`, `dev_assets_seed/`, `dev_assets/`: Packaged and local dev assets.
- `npx-cli/`: Files published to the npm CLI package.
- `scripts/`: Dev helpers (ports, DB preparation).
- `docs/`: Documentation files.

### Crate-specific guides
- [`crates/mcp/AGENTS.md`](crates/mcp/AGENTS.md) — MCP server architecture, launch modes, backend resolution (and the intermittent-availability fix), config propagation to launched agents.
- [`crates/remote/AGENTS.md`](crates/remote/AGENTS.md) — Remote server architecture, ElectricSQL integration, mutation patterns, environment variables.
- [`docs/AGENTS.md`](docs/AGENTS.md) — Mintlify documentation writing guidelines and component reference.
- [`packages/local-web/AGENTS.md`](packages/local-web/AGENTS.md) — Web app design system styling guidelines.

## Managing Shared Types Between Rust and TypeScript

ts-rs allows you to derive TypeScript types from Rust structs/enums. By annotating your Rust types with #[derive(TS)] and related macros, ts-rs will generate .ts declaration files for those types.
When making changes to the types, you can regenerate them using `pnpm run generate-types`
Do not manually edit shared/types.ts, instead edit crates/server/src/bin/generate_types.rs

For remote/cloud types, regenerate using `pnpm run remote:generate-types`
Do not manually edit shared/remote-types.ts, instead edit crates/remote/src/bin/remote-generate-types.rs (see crates/remote/AGENTS.md for details).

## Build, Test, and Development Commands
- Fresh-worktree setup: `pnpm install --frozen-lockfile` (required before
  development and repository verification commands)
- Run dev (web app + backend with ports auto-assigned): `pnpm run dev`
- Backend (watch): `pnpm run backend:dev:watch`
- Web app (dev): `pnpm run local-web:dev`
- Type checks: `pnpm run check` (frontend + all backend Rust workspaces) and `pnpm run backend:check` (all backend Rust workspaces, including `crates/remote`)
- Rust tests: `cargo test --workspace`
- Generate TS types from Rust: `pnpm run generate-types` (or `generate-types:check` in CI)
- Prepare SQLx (offline): `pnpm run prepare-db`
- Prepare SQLx (remote package, postgres): `pnpm run remote:prepare-db`
- Local NPX build: `pnpm run build:npx` then `pnpm pack` in `npx-cli/`
- Format code: `pnpm run format` (runs `cargo fmt` for all backend Rust workspaces + web-core/web Prettier)
- Lint: `pnpm run lint` (runs web/ui ESLint + `cargo clippy` for all backend Rust workspaces)

## Before Completing a Task
- In a fresh worktree, run `pnpm install --frozen-lockfile` before verification.
- Run `pnpm run format` to format all Rust workspaces and web code.

## Coding Style & Naming Conventions
- Rust: `rustfmt` enforced (`rustfmt.toml`); group imports by crate; snake_case modules, PascalCase types.
- TypeScript/React: ESLint + Prettier (2 spaces, single quotes, 80 cols). PascalCase components, camelCase vars/functions, kebab-case file names where practical.
- Keep functions small, add `Debug`/`Serialize`/`Deserialize` where useful.

## Testing Guidelines
- Rust: prefer unit tests alongside code (`#[cfg(test)]`), run `cargo test --workspace`. Add tests for new logic and edge cases.
- Web app: ensure `pnpm run check` and `pnpm run lint` pass. If adding runtime logic, include lightweight tests (e.g., Vitest) in the same directory.

## Dependencies
- Cargo, npm, and GitHub Actions versions are updated by Renovate (see `renovate.json`). Renovate opens PRs automatically and **auto-merges them once CI is green** — except the two carve-outs below, which wait for a human.
- Several agent CLIs are pinned **inside Rust source** as `npx -y <pkg>@<version>` strings (e.g. `@anthropic-ai/claude-code`, `@musistudio/claude-code-router`, `@openai/codex`, `@google/gemini-cli`, `@github/copilot`, `@qwen-code/qwen-code`, `opencode-ai` in `crates/executors/src/executors/*.rs`). These are picked up by a Renovate custom regex manager — **do not hand-bump them**; let Renovate open the PR.
- The bundled **Slack** MCP catalog entry (`crates/executors/default_mcp.json`) pins a GitHub release asset from the `davidvasandani/slack-mcp-server` fork, not an npm package — upstream's `@latest` does not contain the fork's attachment retrieval. Its own Renovate custom manager tracks the release tag; a bump must move the URL, the `SLACK_MCP_FORK_TAG` / `SLACK_MCP_LAUNCHER_SHA256` constants in `crates/executors/src/mcp_config.rs`, and the version documented in `docs/integrations/mcp-server-configuration.mdx` **together** — never one of them alone.
- The bundled **Gmail** MCP catalog entry (`crates/executors/default_mcp.json`) pins a **commit SHA** on the `davidvasandani/Gmail-MCP-Server` fork, installed as a `github:` git spec — the package builds itself through a `prepare` script, so no release artifact is needed. Renovate cannot track a bare SHA on a fork with no releases, so this pin is **bumped by hand**; do not add a custom manager for it, because one would match the pin and then never propose a successor (coverage that looks real and is not). A bump moves the SHA in `default_mcp.json`, the `GMAIL_MCP_FORK_REVISION` constant in `crates/executors/src/mcp_config.rs`, and the revision documented in `docs/integrations/mcp-server-configuration.mdx` **together**. Unlike Slack there is deliberately **no** digest constant and **no** audit workflow: a git commit is content-addressed and cannot be re-pointed, so recording a digest of it and re-checking that on a schedule would assert a hash equals itself — whereas Slack's release-asset URL names a location whose bytes GitHub lets a maintainer replace under a fixed tag. Note the scope: the SHA pins the fork's **source**, not its dependency closure, since a `github:` install runs `prepare` and resolves dependencies from npm at install time.
- Carve-outs that are **not** auto-merged and require human review (labeled `needs-review`): **major** updates (a green suite is weak evidence for a major bump), and **`@anthropic-ai/claude-code`** specifically — bumping it can silently change which model the `opus` / `sonnet` / `haiku` aliases resolve to, which CI does not catch, so review the CLI's release notes before merging.

## Security & Config Tips
- Use `.env` for local overrides; never commit secrets. Key envs: `FRONTEND_PORT`, `BACKEND_PORT`, `HOST` 
- Dev ports and assets are managed by `scripts/setup-dev-environment.js`.

