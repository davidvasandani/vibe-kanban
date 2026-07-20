-- Remote project a workspace is linked to, captured at creation (from the
-- linked issue) or when the workspace is linked/unlinked later. Used to
-- resolve organization-level env vars without a remote round-trip; NULL for
-- local-only workspaces.
ALTER TABLE workspaces ADD COLUMN remote_project_id BLOB;
