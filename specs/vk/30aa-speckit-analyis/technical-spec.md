# Technical spec: keep pipeline design artifacts out of shared root files

Task: `30aa-speckit-analyis`

## Problem

Pipeline stages tell the coding agent to write `SPEC.md`,
`PRIOR_KNOWLEDGE.md` and `IMPLEMENTATION_PLAN.md` at the repository root.
Every task rewrites the same three files, so each PR shows a full rewrite of
the previous task's design notes, and concurrent tasks conflict on them.

Commit `b6f2e5ef` already moved the bundled WikiLLM prompts to
`specs/vk/<task-id>/`, but:

1. Installs that seeded `wikillm.toml` before that commit keep the old
   prompts: pipeline files are user-owned once seeded, and no migration was
   shipped for them (only `parallel-subagents.toml` has one).
2. The bundled Basic pipeline still writes `SPEC.md` / `IMPLEMENTATION_PLAN.md`
   at the root, pinned by a test.
3. Some repositories (for example an infrastructure repo) should not carry
   design artifacts in git at all, even task-scoped ones.

## Requirements

### R1: refresh unmodified historical bundled defaults

- Keep the exact previously shipped bytes of every superseded bundled default
  (`basic.toml`, `wikillm.toml`, `speckit.toml`, `parallel-subagents.toml`).
- During seed reconciliation (under the existing seed lock), replace an
  on-disk bundled file with the current default only when its bytes exactly
  equal one of that file's historical defaults.
- Byte differences (customizations), missing files (deletions) and unknown
  content are preserved. Writes use a synced same-directory temp file plus the
  existing atomic replace primitive.
- Generalize the existing Parallel Sub-Agents refresh into this table rather
  than adding a second mechanism.

### R2: Basic writes task-scoped artifacts

- Basic `spec` → `specs/vk/<task-id>/technical-spec.md`; `plan` →
  `specs/vk/<task-id>/implementation-plan.md`, with the same `<task-id>`
  substitution clause WikiLLM uses.
- The previous Basic bytes become a historical default (R1).

### R3: per-repository "keep pipeline artifacts out of git"

- New repo setting `local_pipeline_artifacts: bool` (default `false`, i.e. the
  current behavior of committing task-scoped artifacts).
- When enabled, workspace setup adds a Vibe Kanban-managed block to the repo's
  `info/exclude` (resolved with `git rev-parse --git-path info/exclude`, so a
  linked worktree writes the shared common dir) that ignores `/specs/vk/`.
  When disabled, setup removes that managed block. Other exclude lines are
  never touched.
- Excludes do not untrack files the repo already tracks; that is documented.
- Bundled artifact prompts tell the agent that if git reports the artifact
  path as ignored, it must leave the file uncommitted rather than force-add
  it.
- Exposed in Settings → Repositories as a checkbox, with generated TS types.

## Non-goals

- Changing the knowledge-base stage (it intentionally commits
  `docs/knowledge-base/`).
- Touching repositories other than Vibe Kanban.
- Untracking artifacts a repo already committed.
