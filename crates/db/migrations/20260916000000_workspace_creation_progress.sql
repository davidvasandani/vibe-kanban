CREATE TABLE workspace_creation_progress (
    workspace_id BLOB PRIMARY KEY NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    phase TEXT NOT NULL CHECK (phase IN ('repositories', 'context', 'placement', 'worktrees', 'execution', 'finalizing')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
