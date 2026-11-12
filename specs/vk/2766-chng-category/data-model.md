# Data model

## `chng_settings` (local SQLite, singleton)
| Column | Type | Notes |
|---|---|---|
| id | INTEGER PK, CHECK(id = 1) | singleton |
| jira_base_url | TEXT NOT NULL DEFAULT '' | `https://…atlassian.net`, no trailing `/` |
| jira_email | TEXT NOT NULL DEFAULT '' | Basic-auth user |
| jql | TEXT NOT NULL DEFAULT '' | empty = default JQL |
| encrypted_jira_token | TEXT NULL | `McpGatewaySecretStore` envelope, AAD `vk-chng-settings|jira_token` |
| hashmal_mcp_url | TEXT NOT NULL DEFAULT '' | e.g. `https://tickets.gravy.works/mcp` |
| encrypted_hashmal_token | TEXT NULL | envelope, AAD `vk-chng-settings|hashmal_token` |
| updated_at | TEXT NOT NULL | |

Host key file: `<asset_dir>/chng_settings_aead_key` (generated on first save;
never read when no secret is stored).

## API types (ts-rs)
- `ChngSettings { jira_base_url, jira_email, jql, default_jql, has_jira_token,
  jira_token_reference?, hashmal_mcp_url, has_hashmal_token,
  hashmal_token_reference?, updated_at? }`
- `UpdateChngSettingsRequest { jira_base_url, jira_email, jql,
  jira_token?: string|null, hashmal_mcp_url, hashmal_token?: string|null }`
  — absent/null keep, `""` clears, other replaces.
- `ChngTicket { key, summary, status, status_category, priority?, created?,
  updated?, url }`
- `ChngTicketList { configured, jql, tickets, truncated }`
- `HashmalChngLookup { state, message?, tracked, manual, sessions,
  transcript? }`, `HashmalLookupState = not_configured | unavailable |
  not_tracked | no_sessions | found`
- `HashmalSession { session_id, title, cwd?, last_active?, running }`
- `HashmalTranscript { session_id, total_turns, turns }`,
  `HashmalTurn { role, text, tool?, truncated }`
