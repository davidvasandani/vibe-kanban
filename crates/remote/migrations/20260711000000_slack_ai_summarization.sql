-- AI summarization for the Slack "Create issue from message" shortcut.
--
-- Optional, opt-in, org-admin-controlled. When enabled with a key set, the
-- shortcut reads the Slack thread and asks Anthropic (claude-haiku-4-5) to
-- draft the issue title/description, swapped into the already-open modal.
--
-- The provider API key is a write-only encrypted credential (AES-256-GCM via
-- JwtService, same handling as the bot token / signing secret) and is nullable:
-- its absence means the feature is fully off. The effective gate is
--   enabled AND ai_summarization_enabled AND encrypted_anthropic_api_key IS NOT NULL

ALTER TABLE organization_slack_configs
    ADD COLUMN encrypted_anthropic_api_key TEXT,
    ADD COLUMN ai_summarization_enabled BOOLEAN NOT NULL DEFAULT FALSE;
