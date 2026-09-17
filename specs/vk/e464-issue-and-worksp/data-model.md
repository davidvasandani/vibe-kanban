# Data model and contract
No schema changes. Local SQLite workspaces.archived and remote Postgres workspaces.archived represent activity. Remote workspaces.issue_id identifies issues, whose status_id references project_statuses. Remote id and local_workspace_id are distinct.

Transition: archived=true -> false; if linked issue status name is Done (case-insensitive) and same-project In progress exists, update issue.status_id and updated_at in the workspace mutation transaction. Existing PATCH workspace request and response remain unchanged.
