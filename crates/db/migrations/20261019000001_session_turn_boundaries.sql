-- The latest coding-agent turn of each session whose turn chain finalized
-- with no queued follow-up started in its place (written by
-- ContainerService::finalize_task). Issue comment delivery may start a new
-- turn only at such a boundary, so it never races the exit monitor's own
-- queued-message handoff. Timestamps are application-written RFC 3339.
CREATE TABLE session_turn_boundaries (
    session_id BLOB PRIMARY KEY NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    turn_execution_id BLOB NOT NULL,
    finalized_at TEXT NOT NULL
);
