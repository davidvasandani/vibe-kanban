# SpecKit Artifact Recovery Map

The shared `specs/vk/a5f8-concat-repeating` path accumulated unrelated task
records. Recovery uses file-level historical snapshots instead of treating the
latest mixed directory as one task.

| Owner | Historical source | Restored destination | Included record |
|---|---|---|---|
| `a5f8-concat-repeating` | `a0a53ecb` | `specs/vk/a5f8-concat-repeating/` | Original repeating-log spec, clarification, research, data model, plan, tasks, normalized-patch contract |
| `vk/5e1e-vk-workspace-cre` | `acfe3cb3` | `specs/vk/5e1e-vk-workspace-cre/` | Workspace-creation spec, clarification, research, data model, plan, tasks, background-creation contract |
| `vk/3488-fix-stale-execut` | `311cc689` | `specs/vk/3488-fix-stale-execut/` | PR #226 spec, clarification, research, data model, plan, tasks, execution-stream contract, review |

Files inherited from still other tasks (`contracts.md`, diagnostics contracts,
and unrelated verification/investigation records) are not attributed to either
restored task. Their original task directories already remain in git history;
they are removed from the latest `a5f8` record rather than copied into a third
incorrect owner.

After exact restoration, only the `Feature dir` line in the two moved specs is
updated to the truthful destination. Task IDs and all substantive historical
content remain unchanged.
