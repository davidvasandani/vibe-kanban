CREATE TABLE mcp_gateway_connections (
    id TEXT PRIMARY KEY NOT NULL,
    owner_user_id TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    server_name TEXT NOT NULL,
    upstream_url TEXT NOT NULL,
    transport TEXT NOT NULL CHECK (transport IN ('http', 'sse')),
    auth_kind TEXT NOT NULL CHECK (auth_kind IN ('oauth', 'cloudflare_service_token_oauth')),
    gateway_token_hash BLOB NOT NULL,
    encrypted_credentials TEXT,
    credential_version INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'disconnected',
    expires_at TEXT,
    last_error_code TEXT,
    connected_at TEXT,
    disconnected_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE(owner_user_id, machine_id, id)
);

CREATE TABLE mcp_gateway_oauth_flows (
    id TEXT PRIMARY KEY NOT NULL,
    connection_id TEXT NOT NULL REFERENCES mcp_gateway_connections(id) ON DELETE CASCADE,
    owner_user_id TEXT NOT NULL,
    machine_id TEXT NOT NULL,
    state_hash BLOB NOT NULL UNIQUE,
    encrypted_transient TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    consumed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);

CREATE INDEX idx_mcp_gateway_flows_expiry ON mcp_gateway_oauth_flows(expires_at);
