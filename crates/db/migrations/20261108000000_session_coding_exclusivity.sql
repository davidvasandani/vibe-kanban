-- Reject new overlaps without rewriting historical executions or claiming an
-- already-live legacy process was stopped. The coordinator admission lock owns
-- queue/start decisions; these triggers are the cross-connection backstop.
CREATE TRIGGER execution_processes_one_coding_insert
BEFORE INSERT ON execution_processes
WHEN NEW.run_reason = 'codingagent' AND NEW.status = 'running'
 AND EXISTS (SELECT 1 FROM execution_processes
             WHERE session_id = NEW.session_id AND run_reason = 'codingagent'
               AND status = 'running')
BEGIN
    SELECT RAISE(ABORT, 'session already has a running coding execution');
END;

CREATE TRIGGER execution_processes_one_coding_update
BEFORE UPDATE OF session_id, run_reason, status ON execution_processes
WHEN NEW.run_reason = 'codingagent' AND NEW.status = 'running'
 AND EXISTS (SELECT 1 FROM execution_processes
             WHERE session_id = NEW.session_id AND run_reason = 'codingagent'
               AND status = 'running' AND id != OLD.id)
BEGIN
    SELECT RAISE(ABORT, 'session already has a running coding execution');
END;
