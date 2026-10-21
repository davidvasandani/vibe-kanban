-- Durable delivery of remote issue comments to local workspace sessions.
-- One row per comment (UNIQUE comment_id) makes delivery at-most-once across
-- repeated requests, retries and restarts. Timestamps are written by the
-- application (chrono RFC 3339) so they compare lexicographically; no SQL
-- defaults, which would use a different format.
CREATE TABLE issue_comment_deliveries (
    id BLOB PRIMARY KEY NOT NULL,
    comment_id BLOB NOT NULL UNIQUE,
    issue_id BLOB NOT NULL,
    workspace_id BLOB REFERENCES workspaces(id) ON DELETE SET NULL,
    session_id BLOB REFERENCES sessions(id) ON DELETE SET NULL,
    execution_process_id BLOB REFERENCES execution_processes(id) ON DELETE SET NULL,
    state TEXT NOT NULL,
    reason TEXT,
    error TEXT,
    source TEXT NOT NULL,
    origin_session_id BLOB,
    hop_count INTEGER NOT NULL DEFAULT 0,
    comment_author_id BLOB,
    comment_created_at TEXT NOT NULL,
    delivered_message TEXT,
    delivered_comment_updated_at TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    allow_stopped_target INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    delivered_at TEXT
);

CREATE INDEX idx_issue_comment_deliveries_issue ON issue_comment_deliveries (issue_id);
CREATE INDEX idx_issue_comment_deliveries_session_state ON issue_comment_deliveries (session_id, state);
CREATE INDEX idx_issue_comment_deliveries_state ON issue_comment_deliveries (state, next_attempt_at);
CREATE INDEX idx_issue_comment_deliveries_execution ON issue_comment_deliveries (execution_process_id);
