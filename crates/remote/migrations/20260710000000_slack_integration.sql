-- Slack workspace connection for the "Create issue from message" shortcut.
-- One Slack workspace per organization, and one organization per Slack
-- workspace (the UNIQUE on slack_team_id is what makes inbound payload
-- routing unambiguous).

CREATE TABLE organization_slack_configs (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id          UUID NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    -- AES-256-GCM ciphertext (JwtService::encrypt_string); never returned by the API.
    encrypted_bot_token      TEXT NOT NULL,
    encrypted_signing_secret TEXT NOT NULL,
    -- Captured from Slack auth.test when the bot token is saved; routing key
    -- for inbound interaction payloads (payload team.id -> config).
    slack_team_id            TEXT NOT NULL UNIQUE,
    slack_team_name          TEXT NOT NULL,
    enabled                  BOOLEAN NOT NULL DEFAULT TRUE,
    -- Attributed as creator_user_id on issues created from Slack
    -- (issues.creator_user_id is NOT NULL on create).
    created_by_user_id       UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_organization_slack_configs_updated_at
    BEFORE UPDATE ON organization_slack_configs
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

-- Replay idempotency for modal submissions: each Slack modal instance
-- (view id) may produce at most one issue. The handler checks before
-- inserting; this index closes the check-then-insert race when the same
-- signed submission is processed concurrently.
CREATE UNIQUE INDEX idx_issues_slack_view_id
    ON issues ((extension_metadata #>> '{slack,view_id}'))
    WHERE extension_metadata #>> '{slack,view_id}' IS NOT NULL;
