ALTER TABLE execution_worker_jobs ADD COLUMN coordinator_id BLOB;
ALTER TABLE execution_worker_jobs ADD COLUMN coordinator_instance_id BLOB;
ALTER TABLE execution_worker_jobs ADD COLUMN recovery_checkpoint TEXT NOT NULL DEFAULT 'initial'
    CHECK (recovery_checkpoint IN ('initial', 'refused', 'ready', 'replacement', 'stopped'));
ALTER TABLE execution_worker_jobs ADD COLUMN readiness_attempts INTEGER NOT NULL DEFAULT 0
    CHECK (readiness_attempts BETWEEN 0 AND 2);
ALTER TABLE execution_worker_jobs ADD COLUMN refusal_detail TEXT;
