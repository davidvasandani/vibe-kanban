# Prior knowledge: 30aa-speckit-analyis

Distilled from `docs/knowledge-base/`.

## prompt-driven-agent-pipelines

- Pipeline stages are prose composed into the task description; there is no
  runtime placeholder resolver. Each artifact prompt must say that `<task-id>`
  is replaced with the identifier from the current task or task branch.
- Task-authored artifacts belong under `specs/vk/<task-id>/`, named by role
  (`technical-spec.md`, `prior-knowledge.md`, `implementation-plan.md`).
- Bundled pipeline TOMLs become user-owned once seeded. To refresh one, keep
  the exact previously shipped bytes and, under the seed lock, replace the
  on-disk file only on an exact byte match, via a synced same-directory temp
  file and the atomic replace primitive. Missing files stay deleted; any byte
  difference counts as customization. Tests must cover exact upgrade,
  one-byte customization, deletion, and idempotent second reconciliation.

## pipeline-settings-editor

- Pipeline files are machine-scoped; the Settings editor sends raw bytes
  unchanged. A migration that rewrites files on disk is picked up by the
  normal raw/status queries; no frontend change is needed for R1.

## clustered-workspace-execution

- Workers run ordinary Git commands in their worktrees and write into shared
  storage. Only worktree *administration* is single-writer. Writing a linked
  worktree's `info/exclude` (which resolves into the shared common dir) is an
  ordinary write, and SpecKit already does it at workspace setup
  (`speckit::ensure_git_excludes`, `git rev-parse --git-path info/exclude`).

## SpecKit precedent (code, not KB)

- `crates/services/src/services/speckit/mod.rs` adds a headed block of
  patterns to `info/exclude` so harness files never appear as untracked
  changes, which matters because the Claude Stop hook demands commits for
  untracked changes.
