-- The git identity an organization's workspaces commit as. Agent hosts write
-- name/email into each workspace worktree's git config and install a
-- pre-commit check that rejects any other author or committer email than
-- `email` or one of `allowed_emails` (agent and bot identities).
-- Not synced via ElectricSQL: read on demand by admins and agent hosts.

CREATE TABLE organization_commit_identities (
    organization_id    UUID PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    name               TEXT NOT NULL,
    email              TEXT NOT NULL,
    allowed_emails     TEXT[] NOT NULL DEFAULT '{}',
    updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_organization_commit_identities_updated_at
    BEFORE UPDATE ON organization_commit_identities
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
