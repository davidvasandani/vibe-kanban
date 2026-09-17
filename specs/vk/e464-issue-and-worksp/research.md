# Research
- Execution startup already clears local archived before dispatch, but does not synchronize immediately. Queue acceptance does not clear it.
- Completion sync currently uses archived from execution context; read current state instead to avoid restoring a stale value.
- Remote WorkspaceRepository::update has no issue side effect or transaction; terminal issue transitions already archive linked workspaces in a transaction.
- ProjectStatusRepository::find_by_name is case-insensitive and project-scoped; use it for In progress.
- Issue-level comments are distinct from workspace agent prompts and are out of scope.
