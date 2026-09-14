CREATE TABLE preview_leases (
    id BLOB PRIMARY KEY NOT NULL,
    token_digest TEXT NOT NULL UNIQUE,
    workspace_id BLOB NOT NULL,
    execution_process_id BLOB NOT NULL UNIQUE,
    target_port INTEGER NOT NULL CHECK (target_port BETWEEN 1024 AND 65535),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (execution_process_id) REFERENCES execution_processes(id) ON DELETE CASCADE
);

CREATE INDEX idx_preview_leases_workspace_active
    ON preview_leases(workspace_id, revoked_at, expires_at);
