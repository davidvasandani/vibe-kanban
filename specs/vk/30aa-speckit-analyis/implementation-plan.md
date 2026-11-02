# Implementation plan: 30aa-speckit-analyis

1. **Historical defaults as files.** Add `assets/pipelines/historical/` with
   the exact bytes of every superseded bundled default, extracted with
   `git show`:
   - `basic-44e7c7ef.toml` (current Basic, superseded by this task)
   - `wikillm-44e7c7ef.toml`, `wikillm-b6f2e5ef.toml`
   - `speckit-84796bb1.toml`, `speckit-b6f2e5ef.toml`
   - `parallel-subagents-12ed6434.toml` (byte-identical to the current
     `LEGACY_PARALLEL_SUBAGENTS` const, verified with `cmp`)
2. **Generalize the refresh** in `crates/services/src/services/pipelines/mod.rs`:
   replace `LEGACY_PARALLEL_SUBAGENTS` + `refresh_legacy_parallel_subagents`
   with a `HISTORICAL_BUNDLED: &[(&str, &[&str])]` table and
   `refresh_unmodified_historical_defaults(dir)`, which for each filename
   replaces the file atomically only when its bytes equal a historical entry.
   It runs where the old refresh ran, inside `ensure_seeded` under the lock.
3. **Prompt updates** (`assets/pipelines/basic.toml`, `wikillm.toml`,
   `speckit.toml`): Basic spec/plan move to `specs/vk/<task-id>/…`; every
   artifact-writing stage gains "if git reports that path as ignored, leave it
   uncommitted and do not force-add it".
4. **Tests** (pipelines mod): each historical version upgrades to the current
   default; one-byte customization preserved; deletion preserved; second
   reconciliation is a no-op; current defaults never appear in the historical
   table; Basic prompts are task-scoped (replace the verbatim-root test).
5. **Repo setting.** Migration `repos.local_pipeline_artifacts INTEGER NOT NULL
   DEFAULT 0`; add the field to `Repo`, `UpdateRepo`, every `Repo` query and
   struct literal; `pnpm run prepare-db`; `pnpm run generate-types`.
6. **Exclude sync.** New `crates/services/src/services/pipelines/git_exclude.rs`:
   `sync_artifact_exclude(worktree, enabled)` resolves
   `git rev-parse --git-path info/exclude`, removes any existing managed block,
   appends it when enabled, writes atomically only on change. Unit tests for
   add, idempotence, removal, preserving unrelated lines, and that git
   actually ignores `specs/vk/x/file.md` in a real temp repo.
7. **Call site.** `LocalContainerService::copy_files_and_images` (runs at
   create, ensure, and cluster create) syncs the exclude for every workspace
   repo, best-effort with a warning.
8. **UI.** Settings → Repositories: checkbox bound to
   `local_pipeline_artifacts` next to the parallel-setup checkbox; i18n keys in
   every locale.
9. **Docs/KB.** Update `docs/knowledge-base/prompt-driven-agent-pipelines.md`
   and the repo settings docs if present.
10. Verify: `cargo test -p services pipelines`, `cargo check --workspace`,
    `pnpm run check`, `pnpm run lint`, `pnpm run format`; Codex review.
