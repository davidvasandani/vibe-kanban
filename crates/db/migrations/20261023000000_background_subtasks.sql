-- Durable ownership and completion outbox for agent-delegated child workspaces.
-- IDs deliberately survive workspace teardown so deleted work reports failure.
CREATE TABLE background_subtasks (
    child_workspace_id BLOB PRIMARY KEY NOT NULL,
    child_issue_id BLOB NOT NULL UNIQUE,
    child_project_id BLOB NOT NULL,
    parent_workspace_id BLOB NOT NULL,
    parent_session_id BLOB NOT NULL,
    parent_issue_id BLOB NOT NULL,
    notification_id BLOB NOT NULL UNIQUE,
    hop_count INTEGER NOT NULL,
    outcome TEXT,
    handed_off_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_background_subtasks_pending ON background_subtasks(handed_off_at);
CREATE INDEX idx_background_subtasks_parent ON background_subtasks(parent_workspace_id);
