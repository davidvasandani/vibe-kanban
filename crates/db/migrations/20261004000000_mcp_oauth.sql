-- OAuth 2.1 authorization server state for remote MCP clients that can only
-- authenticate with OAuth (e.g. ChatGPT connectors). Every secret (client
-- secret, consent token, authorization code, access/refresh token) is stored
-- as a SHA-256 digest; plaintext is never persisted. Timestamps are
-- fixed-width UTC RFC 3339 strings written by the application so they
-- compare lexicographically.
CREATE TABLE mcp_oauth_clients (
    client_id TEXT PRIMARY KEY NOT NULL,
    client_secret_hash BLOB,
    token_endpoint_auth_method TEXT NOT NULL,
    client_name TEXT,
    redirect_uris TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_mcp_oauth_clients_created_at ON mcp_oauth_clients (created_at);

CREATE TABLE mcp_oauth_grants (
    id TEXT PRIMARY KEY NOT NULL,
    client_id TEXT NOT NULL REFERENCES mcp_oauth_clients (client_id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    resource TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT
);

CREATE INDEX idx_mcp_oauth_grants_client_id ON mcp_oauth_grants (client_id);

CREATE TABLE mcp_oauth_authorizations (
    id TEXT PRIMARY KEY NOT NULL,
    client_id TEXT NOT NULL REFERENCES mcp_oauth_clients (client_id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    scope TEXT NOT NULL,
    resource TEXT NOT NULL,
    state TEXT,
    consent_hash BLOB NOT NULL,
    code_hash BLOB UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'exchanged', 'denied')),
    grant_id TEXT REFERENCES mcp_oauth_grants (id) ON DELETE SET NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_mcp_oauth_authorizations_expires_at ON mcp_oauth_authorizations (expires_at);

CREATE TABLE mcp_oauth_tokens (
    token_hash BLOB PRIMARY KEY NOT NULL,
    grant_id TEXT NOT NULL REFERENCES mcp_oauth_grants (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('access', 'refresh')),
    expires_at TEXT NOT NULL,
    revoked_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_mcp_oauth_tokens_grant_id ON mcp_oauth_tokens (grant_id);
CREATE INDEX idx_mcp_oauth_tokens_expires_at ON mcp_oauth_tokens (expires_at);
