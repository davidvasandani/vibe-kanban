# Technical spec: CHNG category (vk/2766-chng-category)

## Request

1. Add CHNG to the left drawer.
2. Let the user customize the Jira search in Settings.
3. When creating a session from a CHNG ticket, first search Hashmal for the
   session that created it and offer to continue that one first.

## Decisions (confirmed with the user)

| # | Decision |
|---|----------|
| 1 | A read-only `hashmal_chng_sessions` MCP tool is added to Hashmal (`sweetgreen/platform-ops`, `apps/hashmal`) as a **separate PR that is opened but not merged by this task**. Vibe Kanban must work without it: until it exists, the lookup reports "Hashmal unavailable" and offers a fresh session. |
| 2 | "Continue" starts a new VK workspace whose first prompt carries the CHNG ticket plus the recent transcript of the chosen Hashmal session. "Open in Hashmal" is not offered: Hashmal has no stable deep link to a box session. A fresh session is always offered too. |
| 3 | CHNG is an item in the left AppBar rail (desktop) and the mobile navigation drawer, under a new "Filters" section. It opens a `/chng` page listing tickets. |
| 4 | A new **Settings → CHNG** section, host-scoped (this VK server, i.e. the operator), stores its own Jira URL, email, API token, JQL and Hashmal MCP URL + bearer token. Secrets may be literals or `op://` references, are write-only, and are encrypted at rest. Default JQL is Hashmal's: `project = CSEQ AND issuetype = CHNG AND reporter = currentUser() ORDER BY created DESC`. |
| 5 | A session from a CHNG ticket opens the normal new-workspace create screen with a prefilled prompt. No VK issue is created. |

## Architecture

### Backend (local server, `crates/`)

- **Migration** `crates/db/migrations/20261108000000_chng_settings.sql`:
  singleton table `chng_settings` (`id = 1`) with `jira_base_url`,
  `jira_email`, `jql`, `encrypted_jira_token`, `hashmal_mcp_url`,
  `encrypted_hashmal_token`, `updated_at`.
- **Model** `crates/db/src/models/chng_settings.rs` (runtime `sqlx::query`,
  no offline cache needed — same as `github_owner_token`).
- **Service** `crates/services/src/services/chng.rs`:
  - Settings view/update. Secrets are sealed with `McpGatewaySecretStore`
    under a new host key `chng_aead_key`, bound to a per-field label.
    Literal values are never returned; `op://` references are returned as
    `*_reference`. Changing a URL without re-entering its secret is rejected
    when a secret is stored (credential stays bound to its URL —
    `wiki/external-connector-sync.md`).
  - Secrets resolve through `github_credentials::resolve_reference`
    (server credential files, never ambient env).
  - `search_tickets()` — Jira Cloud `GET /rest/api/3/search/jql` with
    `nextPageToken`, at most 2 pages × 100, fields
    `summary,status,priority,created,updated`. Basic auth (email + token).
  - `hashmal_lookup(key, session_id?)` — one JSON-RPC `tools/call` to
    `hashmal_chng_sessions` over Streamable HTTP (stateless Hashmal accepts a
    bare call; `MCP-Protocol-Version: 2025-06-18`, `Accept: application/json,
    text/event-stream`). Returns a typed `HashmalChngLookup` with a `state`:
    `not_configured | unavailable | not_tracked | no_sessions | found`.
    Every failure maps to `unavailable` plus a classified message that never
    echoes secrets or upstream bodies.
  - Errors never contain token values.
- **Routes** `crates/server/src/routes/chng.rs`:
  - `GET  /api/chng/settings`, `PUT /api/chng/settings`
  - `GET  /api/chng/tickets`
  - `GET  /api/chng/tickets/{key}/hashmal?session_id=…`
  - `{key}` validated against `^[A-Z][A-Z0-9_]*-[0-9]+$`.
- **Types** exported through `crates/server/src/bin/generate_types.rs`.

### Frontend

- `packages/ui/src/components/AppBar.tsx`: optional `onChngClick` /
  `isChngActive`; new `filters` section with one `icon-button` (CHNG).
- `packages/web-core/src/shared/lib/routes/appNavigation.ts`: destination
  `{ kind: 'chng' }`, optional `goToChng`. Local navigation implements it;
  remote navigation does not (CHNG needs the local server API).
- Route `packages/local-web/src/routes/_app.chng.tsx` → `ChngPage`
  (`packages/web-core/src/pages/chng/`).
- `ChngPage`: list, refresh, empty/not-configured/error states, a "Start
  session" button per row.
- `ChngStartSessionDialog`: calls the Hashmal lookup first, shows matched
  sessions (primary first, running dot, last active), "Continue this session"
  per match, and "Start a new session". Continue fetches that session's
  transcript and seeds create mode via `setCreateModeSeedState` +
  `goToWorkspacesCreate()`.
- `chngPrompt.ts` (pure, Vitest): builds the fresh prompt and the
  continuation prompt (ticket header + transcript excerpt, bounded to
  ~24k characters, newest turns kept).
- `ChngSettingsSection.tsx` registered as `chng` (`group: 'host'`).

### Hashmal (separate PR, not merged here)

`backend/mcp_server/tools.py`: `hashmal_chng_sessions(key, session_id?,
limit=20)`. Read-only. Uses `ChngTicket`/`ChngSessionLink` and
`filters_api.resolve_chng_sessions` (manual link > auto-match; `cleared`
suppresses), `transcripts.descriptors_for`, and
`interface.transcript_at_path`. Never scans transcripts on request. A
`session_id` that is not among the ticket's resolved sessions is refused.

## Security

- Secrets: write-only, AES-GCM at rest, per-field binding, `op://` support,
  never logged, never in errors.
- A stored secret is only ever sent to its stored URL.
- Hashmal transcript text is treated as reference data in the prompt and
  labeled as such.
- Jira and Hashmal URLs must be `https://` (or `http://` on loopback for
  tests).

## Testing

- Rust unit tests: JQL default, URL/secret binding rules, Jira response
  parsing + pagination cap, Hashmal response parsing for each state, key
  validation, error redaction.
- Vitest: `chngPrompt.ts`, AppBar CHNG item.
- Hashmal: Django tests for the new tool (tracked/untracked, manual link,
  cleared, session_id restriction, read-only).
