# Implementation Plan: CHNG category

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context
- Local server: Rust (axum, sqlx/SQLite, reqwest, ts-rs). Types reach the web
  through `crates/server/src/bin/generate_types.rs` → `shared/types.ts`.
- Web: React + TanStack Router/Query in `packages/web-core`, rail in
  `packages/ui`, routes in `packages/local-web`.
- Hashmal: Django app in `sweetgreen/platform-ops` (`apps/hashmal`), MCP over
  stateless JSON-RPC with bearer tokens.

## Architecture & Approach
| Requirement | Where |
|---|---|
| FR-1 rail + mobile drawer | `packages/ui/src/components/AppBar.tsx` (new optional `onChngClick`/`isChngActive`, `filters` section); `packages/web-core/src/pages/root/SharedAppLayout.tsx`; `appNavigation.ts` `{kind:'chng'}` + optional `goToChng`; `packages/local-web/src/app/navigation/AppNavigation.ts`; route `packages/local-web/src/routes/_app.chng.tsx` |
| FR-2/3 list | `packages/web-core/src/pages/chng/ChngPage.tsx` → `GET /api/chng/tickets` → `services::chng::search_tickets` (Jira Cloud `/rest/api/3/search/jql`) |
| FR-4/5 settings | `ChngSettingsSection.tsx` (host group) via `machineClient` → `GET/PUT /api/chng/settings` → `services::chng` + `db::models::chng_settings` (singleton table, sealed secrets, separate host key) |
| FR-6..10 start session | `ChngStartSessionDialog.tsx` → `GET /api/chng/tickets/{key}/hashmal` → `services::chng::hashmal_lookup` (JSON-RPC `tools/call hashmal_chng_sessions`); prompt built by `shared/lib/chngPrompt.ts`; seeded with `setCreateModeSeedState` + `goToWorkspacesCreate` |
| Hashmal lookup | new read-only tool in `apps/hashmal/backend/mcp_server/tools.py` (separate PR) |

## Data Model
See `./data-model.md`.

## Contracts
See `./contracts/chng-api.md` and `./contracts/hashmal-chng-sessions.md`.

## Research Notes
See `./research.md`.

## Constitution Check
- IV: rail presentation stays in `@vibe/ui` (props only); fetching, dialogs
  and prompt building live in web-core. ✅
- X: the dialog holds provisional choice only; nothing is persisted until the
  user submits the create screen. ✅
- XL/XLIII: both client reads use AbortController deadlines (Jira 20 s,
  Hashmal 15 s); a failed refetch keeps the last list. ✅
- XLV: credentials come from the CHNG settings entry, are sent only to their
  stored URL, a stored-but-unreadable secret fails closed, and errors name the
  service and the credential source (literal vs 1Password). No ambient
  fallback exists for these services. ✅
- XLVI: Continue is a fresh VK session whose prompt says so; VK never writes
  a Claude transcript. ✅
- LV: `op://` failures reuse `EnvironmentSecretError` categories. ✅
- LVII: the transcript excerpt is framed as attributed reference content from
  Hashmal (session id, title), not instructions. ✅
- Not storing secrets in `Config` (github-owner-token-routing wiki). ✅

## Risks & Dependencies
- Hashmal tool not merged → lookup is `unavailable`; feature still usable.
- Hashmal's `/mcp` may sit behind Cloudflare Access in some deployments; a
  401/403/HTML response is classified as "rejected the token" / "unexpected
  response" rather than parsed.
- Jira Server/DC (non-Cloud) is not supported by `/search/jql`; Settings help
  text says Jira Cloud.
