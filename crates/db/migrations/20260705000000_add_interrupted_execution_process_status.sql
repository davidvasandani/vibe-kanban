-- Add 'interrupted' to the execution_processes status CHECK constraint.
-- Used for processes that were stopped by a server shutdown/restart (deploy)
-- rather than failing or being killed by the user.

-- 1. Add the replacement column with the wider CHECK
ALTER TABLE execution_processes
  ADD COLUMN status_new TEXT NOT NULL DEFAULT 'running'
    CHECK (status_new IN ('running',
                          'completed',
                          'failed',
                          'killed',
                          'interrupted'));

-- 2. Copy existing values across
UPDATE execution_processes
  SET status_new = status;

-- 3. Drop any indexes that reference status
DROP INDEX IF EXISTS idx_execution_processes_status;
DROP INDEX IF EXISTS idx_execution_processes_session_status_run_reason;

-- 4. Remove the old column (requires 3.35+)
ALTER TABLE execution_processes DROP COLUMN status;

-- 5. Rename the new column back to the canonical name
ALTER TABLE execution_processes
  RENAME COLUMN status_new TO status;

-- 6. Re-create all indexes
CREATE INDEX idx_execution_processes_status ON execution_processes(status);

CREATE INDEX idx_execution_processes_session_status_run_reason
        ON execution_processes (session_id, status, run_reason);
