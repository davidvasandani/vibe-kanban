-- One fine-grained GitHub PAT per repository owner. Values are host-key
-- envelopes (see services::github_owner_tokens); plaintext is never stored.
CREATE TABLE github_owner_tokens (
    id BLOB PRIMARY KEY NOT NULL,
    owner TEXT NOT NULL UNIQUE COLLATE NOCASE,
    encrypted_value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
