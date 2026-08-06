ALTER TABLE workspace_affinity_operations
ADD COLUMN session_transfer_manifest_json TEXT
    CHECK (session_transfer_manifest_json IS NULL OR json_valid(session_transfer_manifest_json));

ALTER TABLE workspace_affinity_operations
ADD COLUMN session_transfer_manifest_sha256 TEXT;

ALTER TABLE workspace_affinity_operations
ADD COLUMN session_transfer_verified_at TEXT;

ALTER TABLE workspace_affinity_operations
ADD COLUMN session_transfer_error_category TEXT;
