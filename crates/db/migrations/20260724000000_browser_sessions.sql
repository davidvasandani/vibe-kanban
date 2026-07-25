CREATE TABLE browser_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    host_id TEXT NOT NULL,
    profile TEXT,
    status TEXT NOT NULL DEFAULT 'starting' CHECK (status IN ('starting', 'running', 'closed', 'failed')),
    current_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    closed_at TEXT,
    expires_at TEXT
);

CREATE INDEX idx_browser_sessions_workspace_status
    ON browser_sessions(workspace_id, status);

CREATE TABLE browser_control_transitions (
    id TEXT PRIMARY KEY NOT NULL,
    browser_session_id TEXT NOT NULL REFERENCES browser_sessions(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL,
    controller_type TEXT NOT NULL CHECK (controller_type IN ('none', 'agent', 'human')),
    execution_id TEXT,
    user_id TEXT,
    connection_id TEXT,
    reason TEXT NOT NULL CHECK (reason IN ('acquire', 'release', 'transfer', 'takeover', 'expired', 'disconnected', 'execution_completed', 'closed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

CREATE INDEX idx_browser_control_transitions_session
    ON browser_control_transitions(browser_session_id, created_at);
