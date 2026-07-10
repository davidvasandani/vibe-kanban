-- Bidirectional Jira <-> VK project sync.
-- One Jira connection per project; one link row per Jira issue ever synced.

CREATE TABLE project_jira_configs (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id              UUID NOT NULL UNIQUE REFERENCES projects(id) ON DELETE CASCADE,
    jira_base_url           TEXT NOT NULL,
    auth_mode               TEXT NOT NULL CHECK (auth_mode IN ('cloud_basic', 'server_pat')),
    jira_email              TEXT,
    -- AES-256-GCM ciphertext (JwtService::encrypt_string); never returned by the API.
    encrypted_credential    TEXT NOT NULL,
    jql                     TEXT NOT NULL,
    enabled                 BOOLEAN NOT NULL DEFAULT FALSE,
    -- Attributed as creator_user_id on issues the sync creates (issues.creator_user_id is NOT NULL on create).
    created_by_user_id      UUID REFERENCES users(id) ON DELETE SET NULL,
    sync_interval_seconds   INTEGER NOT NULL DEFAULT 300
                            CHECK (sync_interval_seconds BETWEEN 60 AND 3600),
    status_mapping          JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Level-triggered "sync now" flag: reconciler runs when this is newer than last_sync_started_at.
    sync_requested_at       TIMESTAMPTZ,
    last_sync_started_at    TIMESTAMPTZ,
    last_sync_completed_at  TIMESTAMPTZ,
    last_sync_error         TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TRIGGER trg_project_jira_configs_updated_at
    BEFORE UPDATE ON project_jira_configs
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE jira_issue_links (
    id                          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    -- Cascade from the config, not just the project: deleting the sync
    -- config removes all links even if a reconciler pass is mid-flight
    -- (a stale pass inserting after the delete hits an FK violation
    -- instead of resurrecting links).
    config_id                   UUID NOT NULL REFERENCES project_jira_configs(id) ON DELETE CASCADE,
    project_id                  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id                    UUID NOT NULL UNIQUE REFERENCES issues(id) ON DELETE CASCADE,
    -- Jira's immutable internal issue id; keys can be renamed, ids cannot.
    jira_issue_id               TEXT NOT NULL,
    jira_issue_key              TEXT NOT NULL,
    jira_browse_url             TEXT NOT NULL,
    -- active: syncing; dormant: left the JQL scope (resumes if it returns);
    -- deleted_remote: 404 from Jira, permanently unlinked.
    link_state                  TEXT NOT NULL DEFAULT 'active'
                                CHECK (link_state IN ('active', 'dormant', 'deleted_remote')),
    -- Last-converged snapshot: the 3-way merge base for change detection and
    -- echo prevention (the reconciler's own writes must not look like user edits).
    last_synced_title           TEXT,
    last_synced_description     TEXT,
    last_synced_status_id       UUID,
    last_synced_jira_status     TEXT,
    last_synced_jira_updated_at TIMESTAMPTZ,
    last_synced_vk_updated_at   TIMESTAMPTZ,
    last_error                  TEXT,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, jira_issue_id)
);

CREATE TRIGGER trg_jira_issue_links_updated_at
    BEFORE UPDATE ON jira_issue_links
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE INDEX idx_jira_issue_links_project_id ON jira_issue_links(project_id);

-- Stream links so boards can render Jira keys live.
SELECT electric_sync_table('public', 'jira_issue_links');
