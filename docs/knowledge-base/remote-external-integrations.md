# Building external integrations on the remote server

Contributing tasks: `fec4-vk-slack-shortcu` (Slack message shortcut; built on
patterns from the Jira sync task `d2aa-sync-vk-and-jira`),
`c02f-jira-sync-format` (Jira/VK description format boundary).

The board the user sees is backed by `crates/remote` (Axum + Postgres +
Electric), **not** the local SQLite server — the local server binds loopback
(`crates/server/src/startup.rs`) and can never receive third-party webhooks.
Any integration that talks to an outside service lives in `crates/remote`
(backend) + `packages/web-core` (settings UI), with API DTOs exported via
`crates/remote/src/bin/generate_types.rs` → `shared/remote-types.ts`
(`pnpm run remote:generate-types`).

## The integration checklist (Jira and Slack both follow it)

1. **Config table** with encrypted credentials: ciphertext columns written
   via `state.jwt.encrypt_string` (AES-256-GCM,
   `crates/remote/src/auth/jwt.rs`), decrypted only at use. One config per
   scope (`UNIQUE project_id` / `UNIQUE organization_id`).
2. **Write-only credential API**: PUT accepts `Option<String>` where `None`
   keeps the stored secret; GET returns `has_credential(s): bool` and
   display metadata only. Never log or echo secrets, including in error
   strings (client error enums stringify Slack/Jira error *codes*, not
   requests).
3. **Authorization**: config CRUD behind org admin —
   `OrganizationRepository::assert_admin` wrapped in a route-local
   `assert_admin` (`routes/organization_env_vars.rs` is the founding
   pattern).
4. **Issue creation from an integration**: `IssueRepository::create`
   requires `creator_user_id` → store `created_by_user_id` on the config
   and attribute created issues to that admin. Provenance goes in
   `issues.extension_metadata` JSONB under a per-integration namespace
   (`"jira"`, `"slack"`) — no issues-table schema change. Initial status =
   first non-hidden `project_statuses` row by `sort_order`; sort order =
   `MAX(sort_order)+1` (accepted benign race, both integrations).
5. **Routers**: `routes/<name>.rs` exposes `router()` (merged into
   `v1_protected`) and, for inbound endpoints, `public_router()` (merged
   into `v1_public` next to `github_app::public_router()`).
6. **Frontend**: a `*SettingsSection.tsx` in
   `packages/web-core/src/shared/dialogs/settings/settings/`, registered in
   `settingsRegistry.tsx` (union + initial-state map + definitions +
   render switch) plus `settings.layout.nav.<id>` en-locale labels; an api
   group in `shared/lib/api.ts` using `makeRemoteRequest` (GET 404 →
   `null`); a react-query hook.

## Inbound webhook/interaction endpoints specifically (Slack learnings)

- **Signature verification template**: `github_app/webhook.rs`
  (hmac + sha2 + subtle + hex, constant-time compare). Slack variant
  (`slack/signature.rs`): HMAC over `v0:{timestamp}:{raw_body}` plus a
  ±300 s timestamp-freshness check; take `now` as a parameter so tests are
  deterministic. Verify against the **raw `Bytes` body** before any form
  decoding.
- **Multi-tenant secret routing**: when the signing secret is per-config,
  parse only the tenant id (`team.id`) from the body to find the config,
  then verify, then act. Parsing pre-verification is fine because it is
  side-effect-free. A tenant with *no* config gets an empty 200 — never
  answer an unverifiable request (e.g. via its `response_url`).
- **Ack deadlines beat everything**: Slack's interaction deadline is 3 s
  and the shared `AppState.http_client` timeout is far longer, so an ack
  must never wait on the DB or an outbound API call. Ack immediately and
  do the work in `tokio::spawn` (modal opening); the only inline work
  before an ack should be a single fast insert when the response semantics
  need it (in-modal validation errors on `view_submission`).
- **Replay idempotency**: signed requests can be replayed within the
  timestamp window. Record the interaction's unique id (Slack modal
  `view.id`) in `extension_metadata` and (a) look it up before insert,
  (b) back it with a **partial unique index on the JSONB expression**
  (`ON issues ((extension_metadata #>> '{slack,view_id}')) WHERE … IS NOT
  NULL`), (c) on insert error, re-check the lookup and treat "already
  exists" as success. The lookup, the stored path, and the index
  expression must match exactly.
- **Report failures to the human who clicked** (constitution): Slack
  `view_submission` supports `{"response_action": "errors", {block_id:
  msg}}` for in-modal errors; post-ack confirmations use
  `chat.postEphemeral` with a DM fallback (`conversations.open` →
  `chat.postMessage`) on `not_in_channel`-class errors.

## Environment/deployment notes

- `SERVER_PUBLIC_BASE_URL` (`AppState.server_public_base_url`) is the
  base for links back into the web app (`/projects/{p}/issues/{id}`) and
  for the inbound request URL third parties must reach. The frontend can
  derive the same URL pre-save via `getRemoteApiUrl()` (`remoteApi.ts`).
- **SQLx without Postgres**: new repositories can use runtime-checked
  `sqlx::query_as::<_, T>` (+ `sqlx::FromRow` on the row struct) to avoid
  needing `pnpm run remote:prepare-db`; detect unique-constraint
  violations via `db_err.constraint() == Some("index_or_constraint_name")`.
- Slack Block Kit hard limits worth remembering: `static_select` ≤ 100
  options, `plain_text_input.initial_value` ≤ 3000 chars, option labels
  ≤ 75 chars, `private_metadata` ≤ 3000 chars; `initial_value: ""` is
  rejected — omit the key instead. Truncate by `chars()`, not bytes.
- A message permalink can be constructed without an API call:
  `https://{team_domain}.slack.com/archives/{channel_id}/p{ts_without_dot}`
  (all fields present in the `message_action` payload).

## Canonical formats at sync boundaries

When two systems use different text formats, convert at the external client
boundary and keep one canonical representation through reconciliation and
snapshots. Jira REST API v2 exposes descriptions as Jira wiki markup, while VK
issues and the rich-text editor use Markdown. `jira/client.rs` therefore
converts raw Jira wiki text to Markdown when it creates `JiraIssueData`, and
converts Markdown back to Jira wiki markup only while building an update
payload.

This boundary keeps `jira/sync.rs` format-agnostic: Jira values, VK values, and
`jira_issue_links.last_synced_description` are all compared as Markdown. It
also makes the write-then-read snapshot path converge instead of detecting
representation-only changes on every pass.

For bounded markup converters:

- parse block structures before inline delimiters so code blocks and tables do
  not receive accidental emphasis or link conversion;
- preserve unknown or malformed constructs literally rather than dropping
  content;
- treat backslashes and escaped table pipes as user data unless they escape the
  exact delimiter currently being parsed;
- test both directions and the supported round trip, including Windows paths,
  literal pipes, null/empty descriptions, and trailing newlines.
