# Implementation plan: CHNG category (vk/2766-chng-category)

Builds on `technical-spec.md` and `prior-knowledge.md`.

## Step 1 — Storage (Rust, `crates/db`)

1. `crates/db/migrations/20261112000000_chng_settings.sql`: singleton
   `chng_settings` table (`id INTEGER PRIMARY KEY CHECK (id = 1)`).
2. `crates/db/src/models/chng_settings.rs`: `ChngSettingsRow` with a redacting
   `Debug`, `get(pool)` and `upsert(pool, &row)` via runtime `sqlx::query`.
   Register in `models/mod.rs`.

## Step 2 — Service (Rust, `crates/services/src/services/chng.rs`)

1. Host key path `utils::assets::chng_settings_key_path()` →
   `chng_settings_aead_key`.
2. Types (ts-rs): `ChngSettings`, `UpdateChngSettingsRequest`,
   `ChngTicket`, `ChngTicketList`, `HashmalChngLookup`, `HashmalLookupState`,
   `HashmalSession`, `HashmalTranscript`, `HashmalTurn`.
3. Settings: `get_settings`, `update_settings`.
   - URL validation: `https` (or `http` on loopback), no credentials in URL,
     no query/fragment; trailing `/` trimmed.
   - Secret field semantics on update: `None` keep, `Some("")` clear,
     `Some(v)` replace (normalized like GitHub owner tokens).
   - Destination pinning: changing a URL while a secret stays stored and none
     is supplied → `ChngError::ReenterSecret`.
   - JQL: trimmed, non-empty after trim or falls back to the default, ≤ 2000 chars.
4. Jira: `search_tickets(pool, http)`; pure helpers `parse_search_page`,
   `browse_url`. Pagination: up to `MAX_PAGES = 2`, `PAGE_SIZE = 100`,
   `truncated` set when `nextPageToken` remains. Errors classified:
   not configured / unauthorized (names Jira + credential source) / bad JQL
   (Jira's `errorMessages` are JQL feedback, returned bounded to 300 chars) /
   unreachable / unexpected response.
5. Hashmal: `hashmal_lookup(pool, http, key, session_id)`; pure
   `parse_hashmal_result(json) -> HashmalChngLookup`. Never returns `Err` to
   the route: every failure becomes `state = unavailable` with a classified
   message (unknown tool → "Hashmal does not have the CHNG lookup yet").
6. Secret resolution: `github_credentials::resolve_reference` (make it
   `pub(crate)` reachable — it already is within `services`).
7. Unit tests for every pure helper and the update rules (using
   `McpGatewaySecretStore` with a temp key file and an in-memory SQLite pool,
   following existing service tests).

## Step 3 — Routes (Rust, `crates/server`)

1. `crates/server/src/routes/chng.rs`: four handlers, key validation, 10 s
   timeout per outbound call via the shared `reqwest::Client` builder.
2. Register in `routes/mod.rs`; map `ChngError` in `error.rs` (400 for
   validation, 502 for upstream, 500 otherwise).
3. Export types in `crates/server/src/bin/generate_types.rs`, run
   `pnpm run generate-types`.

## Step 4 — Frontend data layer (`packages/web-core`)

1. `shared/lib/api.ts`: `chngApi.listTickets(signal)`,
   `chngApi.lookupHashmal(key, sessionId?, signal)` with `AbortController`
   deadlines (XL/XLIII).
2. `shared/lib/machineClient.ts`: `getChngSettings`, `updateChngSettings`.
3. `shared/lib/chngPrompt.ts` + `chngPrompt.test.ts`.

## Step 5 — Navigation + left rail

1. `packages/ui/src/components/AppBar.tsx`: `onChngClick?`, `isChngActive?`,
   `filters` section with the CHNG `icon-button` (only when `onChngClick`).
2. `AppBar.test.tsx`: renders the CHNG item and fires the handler; absent
   without the handler.
3. `appNavigation.ts`: `{ kind: 'chng' }`, `goToChng?`.
   `local-web/src/app/navigation/AppNavigation.ts`: resolve + navigate.
4. `local-web/src/routes/_app.chng.tsx`; regenerate `routeTree.gen.ts`
   (vite plugin / `tsr generate`).
5. `SharedAppLayout.tsx`: pass `onChngClick`/`isChngActive` when
   `appNavigation.goToChng` exists; add a CHNG link in the mobile drawer.
6. i18n: `common.json` `appBar.chng.*` (en + other locales fallback to en).

## Step 6 — CHNG page + start-session dialog (`packages/web-core/src/pages/chng`)

1. `ChngPage.tsx`: React Query list (`['chng','tickets']`), refresh,
   states: loading, not configured (button → Settings CHNG), error, empty,
   list with truncated notice. Row: key (external link), summary, status,
   updated, "Start session".
2. `ChngStartSessionDialog.tsx` (nice-modal): on open, lookup Hashmal
   (no `session_id`). Shows found sessions (primary first, running dot,
   last-active time, "manual link" tag) each with "Continue"; "Start a new
   session" always present. Non-found states show a one-line reason.
   Continue: if the chosen session is not the one whose transcript was
   returned, fetch it with `session_id`. Then `setCreateModeSeedState(
   { initialPrompt })` + `goToWorkspacesCreate()`.

## Step 7 — Settings section

1. `ChngSettingsSection.tsx` using `useSettingsMachineClient`, fields:
   Jira URL, email, API token (write-only, shows "stored"/reference, clear),
   JQL (textarea + "Reset to default"), Hashmal MCP URL, Hashmal token.
   Dirty tracking through `SettingsDirtyContext`; save via PUT.
2. Register in `SettingsRegistry.tsx` (`chng`, `group: 'host'`) and add nav
   label `settings.layout.nav.chng` in locales.

## Step 8 — Hashmal PR (separate repo, opened not merged)

1. `apps/hashmal/backend/mcp_server/tools.py`: `chng_sessions` +
   `hashmal_chng_sessions` registry entry.
2. Tests in `apps/hashmal/backend/mcp_server/tests.py`.
3. README MCP table row.

## Step 9 — Verify

`pnpm install --frozen-lockfile`, `cargo test -p services chng`,
`cargo test -p server`, `pnpm run generate-types:check`, `pnpm run check`,
`pnpm run lint`, Vitest for touched packages, `pnpm run format`.
