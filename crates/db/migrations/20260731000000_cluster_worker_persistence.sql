CREATE TABLE worker_nodes (
    id                     BLOB PRIMARY KEY,
    hostname               TEXT NOT NULL,
    status                 TEXT NOT NULL DEFAULT 'offline'
                               CHECK (status IN ('online', 'offline', 'draining')),
    worker_version         TEXT NOT NULL,
    vibe_version           TEXT NOT NULL,
    capabilities           TEXT NOT NULL DEFAULT '{}'
                               CHECK (json_valid(capabilities)),
    resource_snapshot      TEXT NOT NULL DEFAULT '{}'
                               CHECK (json_valid(resource_snapshot)),
    labels                 TEXT NOT NULL DEFAULT '{}'
                               CHECK (json_valid(labels)),
    mount_status           TEXT NOT NULL DEFAULT 'missing'
                               CHECK (mount_status IN (
                                   'healthy',
                                   'missing',
                                   'local_fallback',
                                   'wrong_filesystem',
                                   'probe_not_visible',
                                   'read_only',
                                   'ownership_mismatch',
                                   'io_error'
                               )),
    mount_message          TEXT,
    last_heartbeat_at      TEXT,
    lease_expires_at       TEXT,
    created_at             TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at             TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

CREATE UNIQUE INDEX idx_worker_nodes_hostname ON worker_nodes(hostname);
CREATE INDEX idx_worker_nodes_schedulable
    ON worker_nodes(status, mount_status, lease_expires_at);
CREATE INDEX idx_worker_nodes_lease_expires_at
    ON worker_nodes(lease_expires_at)
    WHERE lease_expires_at IS NOT NULL;

ALTER TABLE workspaces
    ADD COLUMN worker_node_id BLOB REFERENCES worker_nodes(id);
ALTER TABLE workspaces
    ADD COLUMN placement_state TEXT NOT NULL DEFAULT 'local'
        CHECK (placement_state IN (
            'local',
            'reserved',
            'provisioning',
            'ready',
            'failed'
        ));
ALTER TABLE workspaces ADD COLUMN placed_at TEXT;
ALTER TABLE workspaces ADD COLUMN placement_reason TEXT;
ALTER TABLE workspaces
    ADD COLUMN requested_worker_node_id BLOB REFERENCES worker_nodes(id);
ALTER TABLE workspaces
    ADD COLUMN placement_constraints TEXT
        CHECK (
            placement_constraints IS NULL
            OR json_valid(placement_constraints)
        );

CREATE INDEX idx_workspaces_worker_node_id
    ON workspaces(worker_node_id)
    WHERE worker_node_id IS NOT NULL;
CREATE INDEX idx_workspaces_placement_state
    ON workspaces(placement_state);
CREATE INDEX idx_workspaces_requested_worker_node_id
    ON workspaces(requested_worker_node_id)
    WHERE requested_worker_node_id IS NOT NULL;

CREATE TABLE execution_worker_jobs (
    execution_process_id   BLOB PRIMARY KEY,
    worker_node_id         BLOB NOT NULL,
    worker_job_id          BLOB NOT NULL,
    request_digest         TEXT NOT NULL,
    dispatch_state         TEXT NOT NULL DEFAULT 'pending'
                               CHECK (dispatch_state IN (
                                   'pending',
                                   'accepted',
                                   'starting',
                                   'running',
                                   'cancelling',
                                   'completed',
                                   'failed',
                                   'killed',
                                   'interrupted',
                                   'indeterminate',
                                   'quarantined'
                               )),
    last_event_sequence    INTEGER NOT NULL DEFAULT 0
                               CHECK (last_event_sequence >= 0),
    worker_last_sequence   INTEGER NOT NULL DEFAULT 0
                               CHECK (worker_last_sequence >= 0),
    lease_expires_at       TEXT,
    output_complete        INTEGER NOT NULL DEFAULT 1
                               CHECK (output_complete IN (0, 1)),
    terminal_evidence      TEXT
                               CHECK (
                                   terminal_evidence IS NULL
                                   OR json_valid(terminal_evidence)
                               ),
    dispatched_at          TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    accepted_at            TEXT,
    completed_at           TEXT,
    created_at             TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at             TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (execution_process_id)
        REFERENCES execution_processes(id) ON DELETE CASCADE,
    FOREIGN KEY (worker_node_id)
        REFERENCES worker_nodes(id),
    UNIQUE (worker_node_id, worker_job_id)
);

CREATE INDEX idx_execution_worker_jobs_worker_state
    ON execution_worker_jobs(worker_node_id, dispatch_state);
CREATE INDEX idx_execution_worker_jobs_lease_expires_at
    ON execution_worker_jobs(lease_expires_at)
    WHERE lease_expires_at IS NOT NULL;

CREATE TABLE repository_admin_locks (
    repo_id                BLOB PRIMARY KEY,
    generation             INTEGER NOT NULL CHECK (generation >= 0),
    operation_id           BLOB NOT NULL,
    acquired_at            TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    lease_expires_at       TEXT NOT NULL,
    FOREIGN KEY (repo_id) REFERENCES repos(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_repository_admin_locks_operation_id
    ON repository_admin_locks(operation_id);
CREATE INDEX idx_repository_admin_locks_lease_expires_at
    ON repository_admin_locks(lease_expires_at);
