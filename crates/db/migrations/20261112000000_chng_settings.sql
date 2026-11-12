-- Settings → CHNG: the operator's Jira search for change tickets and the
-- Hashmal MCP endpoint used to find the session that worked a ticket.
-- One row per host. Tokens are host-key envelopes (see services::chng);
-- plaintext is never stored.
CREATE TABLE chng_settings (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
    jira_base_url TEXT NOT NULL DEFAULT '',
    jira_email TEXT NOT NULL DEFAULT '',
    jql TEXT NOT NULL DEFAULT '',
    encrypted_jira_token TEXT,
    hashmal_mcp_url TEXT NOT NULL DEFAULT '',
    encrypted_hashmal_token TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
