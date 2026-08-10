# Pipeline Prompt Contract

## WikiLLM

| Stage ID | Required destination |
| --- | --- |
| `spec` | `specs/vk/<task-id>/technical-spec.md` |
| `recall-knowledge` | `specs/vk/<task-id>/prior-knowledge.md` |
| `plan` | `specs/vk/<task-id>/implementation-plan.md` |

Every affected prompt explains that `<task-id>` is replaced with the current
task identifier derived from the task or task branch. The stage does not write
its artifact to a shared repository or workspace root filename.

## SpecKit constitution

At constitution draft time, the agent marks a new principle number provisional.
Immediately before merge, the WikiLLM and SpecKit merge contracts require the
agent to:

1. inspects the latest tip of the actual base branch (`main` only when it is the
   base branch);
2. finds the highest existing principle number there;
3. selects the next free number; and
4. renumber its own unmerged addition and update its internal references if the
   provisional number is already occupied, without renumbering an already-merged
   principle.

## Compatibility

Stage ordering, IDs, labels, `default_enabled`, `heavy`, pipeline file schema,
and all unrelated prompt fragments are unchanged.
