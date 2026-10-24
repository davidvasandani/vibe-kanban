-- No target foreign keys: cancellation/audit must survive target deletion.
CREATE TABLE wakeups (
    id BLOB PRIMARY KEY NOT NULL,
    workspace_id BLOB NOT NULL,
    session_id BLOB NOT NULL,
    fire_at TEXT NOT NULL,
    prompt TEXT NOT NULL,
    poller_id BLOB,
    receipt_path TEXT,
    trigger_context TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX wakeups_session ON wakeups(session_id);
