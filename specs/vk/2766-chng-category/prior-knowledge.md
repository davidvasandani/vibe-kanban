# Prior knowledge for vk/2766-chng-category

Knowledge base: `wiki/` (64 pages, index `wiki/INDEX.md`). Read-only here.

## Left rail (`wiki/appbar-rail-and-org-tiles.md`, `wiki/left-drawer-space-fit.md`)

- The "left drawer" users see is the **AppBar rail** (`packages/ui/src/components/AppBar.tsx`),
  presentational. Sections are a data array (`AppBarSection[]`) rendered by
  `renderSectionItem`; reuse the `icon-button` kind and the 40×40 tile recipe,
  wrapped in a right-side `Tooltip`.
- Optional props on an exported component need a working fallback; never
  render a button with a no-op `onClick`. → The CHNG section is only pushed when
  `onChngClick` is supplied (remote-web does not supply it).
- The 300px workspaces drawer has a space-fit rule; adding a rail item does
  not affect it (the rail is outside the measured root).
- `@vibe/ui` component tests live in `packages/web-core/src/shared/components/ui-new/AppBar.test.tsx`.

## Settings (`wiki/settings-drawer.md`, `wiki/github-owner-token-routing.md`)

- Settings is a docked right drawer; sections come from `SettingsRegistry.tsx`.
  A section's cleanup must clear its dirty flag on unmount.
- **Never store secrets in `Config`**: `GET /api/info` returns the whole Config
  to the browser and `PUT /api/config` writes it back. Machine-scoped secrets
  get their own SQLite table, runtime `sqlx::query_as` (no `.sqlx` cache
  change), `McpGatewaySecretStore` envelopes under a **separate host key file**,
  AAD-bound, write-only literals, only `op://` references returned.
- Server-side `op://` resolution must use the credential files
  (`server_credentials()`), never ambient env — `github_credentials::resolve_reference`.

## External connectors (`wiki/external-connector-sync.md`, `wiki/1password-reference-resolution.md`)

- **A stored credential may only be sent to its stored destination**: changing
  the base URL requires re-entering the credential, enforced atomically with
  the write.
- Jira Cloud removed legacy `/rest/api/2/search` — use `/search/jql` +
  `nextPageToken`. Jira timestamps are `%Y-%m-%dT%H:%M:%S%.f%z`.
- `op` failure messages are classified, never echoed (stderr leaks
  vault/item names).
- There is no existing Jira search in the local server; Jira sync lives in the
  separate `crates/remote` workspace and cannot be reused from `crates/services`.

## Create mode (`wiki/create-mode-repo-branch-defaulting.md`)

- Prefilled workspace creation: `setCreateModeSeedState({ initialPrompt })`
  then `appNavigation.goToWorkspacesCreate()` (the "duplicate workspace"
  action does this). The user still picks repos and agent.
- No VK API continues a session from another host/session; continuation must
  be done by seeding context into the new prompt.

## Hashmal (outside this repo — `sweetgreen/platform-ops` `apps/hashmal`)

- Already ships a CHNG filter (`docs/superpowers/specs/2026-06-29-hashmal-filters-session-linking-design.md`):
  `ChngTicket.matched_session_ids` (transcripts mentioning the key, newest
  first) + `ChngSessionLink` manual override (`cleared` suppresses auto-match),
  merged by `filters_api.resolve_chng_sessions`.
- `/api/filters/*` uses Django session auth only. The MCP endpoint
  (`/mcp`, stateless JSON-RPC, bearer `hsm_…` tokens, no SSE) is the only
  token-authenticated surface; it has no CHNG tool yet.
- Never scan transcripts on a request (cold scans hang workers).

## Gaps

- No KB page on CHNG, Hashmal, or rail items that open full pages. This task
  should record the Hashmal contract and the "continuation by prompt seeding"
  decision.
