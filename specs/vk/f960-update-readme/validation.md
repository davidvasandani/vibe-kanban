# Validation

Task: f960-update-readme.

- Installed dependencies with `pnpm install --frozen-lockfile` in this fresh worktree. Dependency tree exists and `node scripts/check-format-prerequisites.mjs` passes.
- README relative link check: all 58 local targets resolve. Ancestry appears in one footnote; the only other use of fork terminology is the agent session-forking capability.
- `git diff --check`: passed.
- Setup commands checked against package.json, rust-toolchain.toml, local-build.sh, server startup configuration and Compose definitions.
- Feature evidence and limitations recorded in research.md.
- `pnpm run format`: passed across both Rust manifests and all configured frontend packages; no runtime files changed.
- Runtime tests are not needed for this documentation-only diff. No source build or deployment was performed.

- README parsed successfully using the installed Prettier Markdown parser. Root script references and all 17 literal architecture paths checked successfully.
- Independent Codex review: no actionable findings, exit 0. See review.md.
