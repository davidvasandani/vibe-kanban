-- A remote execution is indeterminate when the coordinator cannot verify
-- whether its worker process is still running or how it terminated.
ALTER TABLE execution_processes
  ADD COLUMN status_new TEXT NOT NULL DEFAULT 'running'
    CHECK (status_new IN ('running',
                          'completed',
                          'failed',
                          'killed',
                          'interrupted',
                          'indeterminate'));

UPDATE execution_processes SET status_new = status;

DROP INDEX IF EXISTS idx_execution_processes_status;
DROP INDEX IF EXISTS idx_execution_processes_session_status_run_reason;

ALTER TABLE execution_processes DROP COLUMN status;
ALTER TABLE execution_processes RENAME COLUMN status_new TO status;

CREATE INDEX idx_execution_processes_status ON execution_processes(status);
CREATE INDEX idx_execution_processes_session_status_run_reason
        ON execution_processes (session_id, status, run_reason);
