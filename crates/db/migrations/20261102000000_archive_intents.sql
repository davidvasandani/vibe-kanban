CREATE TABLE workspace_archive_intents (
    workspace_id BLOB NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    trigger_key TEXT NOT NULL,
    rule TEXT NOT NULL,
    triggered_at TEXT NOT NULL,
    archived_at TEXT,
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (workspace_id, trigger_key)
);
CREATE TABLE workspace_archive_overrides (
    workspace_id BLOB PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    unarchived_at TEXT NOT NULL
);
