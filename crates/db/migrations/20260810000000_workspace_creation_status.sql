ALTER TABLE workspaces
ADD COLUMN creation_status TEXT NOT NULL DEFAULT 'ready'
CHECK (creation_status IN ('queued', 'running', 'ready', 'failed'));

ALTER TABLE workspaces ADD COLUMN creation_error TEXT;
