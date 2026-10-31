# Prompt-driven agent pipelines

Tags: `a17f-fix-parallel-age`, `89c5-pipeline-instruc`, `30aa-speckit-analyis`

## Treat prompts as executable contracts

Bundled pipeline stages are composed into a coding agent's task description;
there is no separate runtime interpreting their intent. Reliability-sensitive
wording therefore needs the same contract discipline as code.

For cross-provider fan-out, state the positive execution contract explicitly:

- launch independent providers concurrently;
- give each child the unchanged original task as its initial input;
- retain workspace-reading tools while using non-mutating instructions and a
  read-only sandbox or permission policy where available;
- wait for complete, provider-labeled final responses;
- isolate launch, authentication, timeout, and availability failures;
- never synthesize a fake provider response or spend completed-round budget on
  failed launches;
- use fresh children on later rounds, preserving the original prompt and adding
  the previous synthesis as clearly separated context.

"Read-only" must not be implemented as an empty tool set. A technical analyst
without repository-reading capability can decline before understanding the
task or return an ungrounded partial answer. Tests should pin these semantic
clauses, not merely check that provider names or the word `parallel` appear.

## Scope task-authored artifacts by task identity

Concurrent tasks in one repository must never be instructed to write design
records to shared root filenames. Put WikiLLM artifacts beside the task's
SpecKit record under `specs/vk/<task-id>/`, and name each artifact by role (for
example, `technical-spec.md`, `prior-knowledge.md`, and
`implementation-plan.md`). Because pipeline fragments are executable prose and
there is no placeholder resolver, each prompt must explain that `<task-id>` is
replaced with the identifier from the current task or task branch.

Constitution principle numbers assigned on a task branch are provisional. An
early draft-time base check cannot prevent another concurrent branch from
merging the same number first. The merge-stage contract must re-read the latest
actual base-branch tip immediately before integration, renumber only the
unmerged addition to the next free number, and update its internal references.
Already-merged principles remain stable because external documents may cite
their numbers.

## Refresh user-editable bundled defaults safely

Changing an embedded bundled asset does not update installations that already
recorded that filename in `.bundled-pipelines.json`. At the same time, pipeline
TOMLs become user-owned once copied into editable storage, so known filenames
cannot simply be overwritten.

For a narrow correction with one known predecessor, keep the exact previously
shipped bytes as private migration data. Under the existing seed lock:

1. Read the on-disk target if present.
2. Replace it only when its bytes exactly equal the historical default.
3. Write and sync a same-directory temporary file, then use the existing atomic
   replace primitive.
4. Preserve missing files as deletions and any byte difference as customization
   or unknown state.

This migration is naturally idempotent because the new default no longer
matches the historical bytes. Regression coverage should prove exact upgrade,
one-byte customization preservation, deletion preservation, and a second
reconciliation with no further change.

Changing a bundled default *without* recording its predecessor strands every
install seeded earlier: `b6f2e5ef` moved WikiLLM to task-scoped paths, but
existing installs kept writing root `SPEC.md` until the predecessor bytes were
added. The predecessors now live as exact `git show` copies in
`assets/pipelines/historical/<name>-<commit>.toml`, listed in
`HISTORICAL_BUNDLED`. Any edit to a bundled TOML must add the previous bytes
there; a test asserts no current default is listed as historical and every
listed version upgrades. Never reformat those files, since a single changed
byte stops matching.

## Keeping artifacts out of git per repository

Task-scoped directories turn root-file rewrites into additions, but some
repositories should not carry design records at all. The repo setting
`local_pipeline_artifacts` is enforced by a managed block in the repo's
`info/exclude`, not by prompt prose, because the Claude Stop hook demands
commits for untracked changes and agents commit whatever `git status` shows.
Lessons from review:

- Resolve the file with `git rev-parse --git-path info/exclude`. In a linked
  worktree it is the shared common-dir file, so the block is repo-wide.
- A pattern containing a middle `/` is anchored to the repo root. Artifacts
  can sit below a repo working directory, so use `**/specs/vk/`.
- SpecKit files records under `specs/<feature-key>/`, where the key is the
  *persisted* `speckit_feature_key` (it survives branch renames), falling back
  to the branch. Cover both keys' first components.
- Because the file is shared, branch-derived patterns must accumulate, not be
  replaced by whichever workspace synced last, and the read/merge/write needs
  an exclusive lock: atomic rename prevents torn writes but not lost updates.
- Prompts still say "if git reports the path as ignored, never force-add it",
  since an explicit `git add <path>` fails on ignored files and invites `-f`.
- Excludes never untrack files a repo already committed.
