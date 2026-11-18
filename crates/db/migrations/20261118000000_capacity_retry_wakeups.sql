-- Automatic capacity backoff: a wake-up scheduled by Vibe Kanban itself after
-- a coding-agent turn ended with a provider capacity error. `capacity_retry_of`
-- is the execution that failed; the unique index makes scheduling idempotent
-- per failed turn. Both columns are NULL for agent wake-ups and poller
-- notifications.
ALTER TABLE wakeups ADD COLUMN capacity_retry_of BLOB;
ALTER TABLE wakeups ADD COLUMN capacity_attempt INTEGER;
CREATE UNIQUE INDEX wakeups_capacity_retry_of
    ON wakeups(capacity_retry_of)
    WHERE capacity_retry_of IS NOT NULL;
