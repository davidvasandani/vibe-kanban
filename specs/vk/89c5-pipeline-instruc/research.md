# Research: Task-Scoped Pipeline Design Records

## Decision: prompts carry the path contract

Pipeline fragments are composed directly into issue descriptions. There is no
execution-time path interpolation layer, so adding a new placeholder type or
runtime resolver would be speculative and would not help external agents reading
the task text. The prompt will name the path and explain how the agent derives
the current task ID.

## Decision: scope all three WikiLLM artifacts

Moving only the committed technical spec and implementation plan would still
allow concurrent tasks to overwrite workspace-root prior knowledge and would
split one task's design record across directories. Stage 2 therefore uses
`specs/vk/<task-id>/prior-knowledge.md` as well.

## Decision: base branch, not hard-coded `main`

The reported incidents use `main`, but Vibe Kanban tasks can target other base
branches. The contract says latest base-branch tip and gives `main` as an
example. This preserves the collision rule for every repository.

## Decision: no automatic bundled-file migration

Machine-local pipeline TOMLs are user-editable. Project knowledge requires
exact predecessor bytes before an automatic migration can distinguish an
untouched default from customization. The requested prompt correction will ship
in embedded defaults and through the existing reset action; seeding semantics
remain unchanged.

## Alternatives rejected

- Repository-root filenames suffixed with task IDs: keeps task artifacts
  scattered and does not colocate them with SpecKit output.
- A shared `specs/vk/PRIOR_KNOWLEDGE.md`: still collides across tasks.
- Renumbering merged principles after detecting a collision: breaks stable
  external citations; only the unmerged addition is free to move.

No new dependency is required.
