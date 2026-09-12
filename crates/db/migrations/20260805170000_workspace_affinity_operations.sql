CREATE TABLE workspace_affinity_operations (
    operation_id             BLOB PRIMARY KEY,
    workspace_id             BLOB NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source_execution_id      BLOB REFERENCES execution_processes(id),
    source_stop_started      INTEGER NOT NULL DEFAULT 0 CHECK (source_stop_started IN (0, 1)),
    requested_worker_node_id BLOB REFERENCES worker_nodes(id),
    restart_running          INTEGER NOT NULL CHECK (restart_running IN (0, 1)),
    status                   TEXT NOT NULL DEFAULT 'claimed'
                                  CHECK (status IN ('claimed', 'completed', 'failed')),
    result_json              TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    error_message            TEXT,
    created_at               TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at               TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

CREATE UNIQUE INDEX idx_workspace_affinity_active
    ON workspace_affinity_operations(workspace_id)
    WHERE status = 'claimed';
