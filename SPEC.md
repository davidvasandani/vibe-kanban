# Technical Spec: Bidirectional Jira ↔ VK Project Sync

> Task d2aa. Full SpecKit artifacts live in
> `homelab/specs/vk/d2aa-sync-vk-and-jira/` (`spec.md`, `plan.md`,
> `research.md`, `data-model.md`, `contracts/`, `tasks.md`). This file is the
> repo-root technical summary.

## Problem

Work is planned in Jira but executed on VK project boards; moving items
between the two is manual copy-paste in both directions, and the two views
silently drift.

## Solution

Connect one VK project to one Jira instance via a JQL query, configured in
the VK Project Settings dialog. Jira issues matching the query appear as
board issues; changes to the synced fields (title, description, status) flow
both ways through a periodic server-side reconciler with visible status and
per-issue error reporting. Sync never deletes VK issues; disconnecting keeps
the board intact.

## Where it lives

The board in this fork is the **remote** stack (`crates/remote`: Axum +
Postgres + ElectricSQL), so the whole feature is implemented there plus the
shared frontend (`packages/web-core`, `packages/ui`).

### Backend (`crates/remote`)

- **Schema** — `migrations/20260709000000_jira_sync.sql`:
  `project_jira_configs` (one per project; AES-256-GCM-encrypted credential
  via the existing `JwtService`; JQL; enabled flag; interval; JSONB status
  mapping; sync-state stamps; `created_by_user_id` for attributing created
  issues) and `jira_issue_links` (one per synced Jira issue, keyed by Jira's
  immutable internal id — `UNIQUE(project_id, jira_issue_id)` makes import
  idempotent; `link_state` active/dormant/deleted_remote; last-synced
  snapshot columns; per-link `last_error`). Links are electrified for live
  board badges.
- **Jira client** — `src/jira/client.rs`: API v2 string semantics (no ADF).
  Cloud (`cloud_basic`, email+token Basic auth) searches via
  `/rest/api/2/search/jql` (`nextPageToken`); Server/DC (`server_pat`,
  Bearer) via classic `/rest/api/2/search`. Field updates via issue PUT,
  status changes via the transitions API, existence checks via issue GET
  (404 ⇒ deleted). Error strings never contain the credential.
- **Status mapping** — `src/jira/mapping.rs`: Jira→VK resolves per-status
  overrides first, then Jira status-*category* defaults (new→"To do",
  indeterminate→"In progress", done→"Done"); VK→Jira is an explicit table,
  auto-seeded from observed statuses, never guessed.
- **3-way merge** — `src/jira/merge.rs`: each synced field is compared per
  side against the link's last-synced snapshot. Only-Jira-moved ⇒ write VK;
  only-VK-moved ⇒ write Jira; both ⇒ last-write-wins with Jira winning
  ties. Snapshots update in the same transaction as the VK write, so the
  reconciler's own writes never echo. After writing to Jira the issue is
  re-read so the snapshot records Jira's normalized representation.
- **Reconciler** — `src/jira/sync.rs`, spawned in `app.rs`: a 30 s global
  ticker (env `JIRA_SYNC_TICK_SECS`) runs a pass for every *due* config
  (interval elapsed, never synced, or level-triggered "sync now" flag).
  Per-issue failures are recorded on the link and aggregated into
  `last_sync_error` without aborting the pass. Scope-out handling: issues
  that leave the JQL become `dormant` (resume on the same VK issue if they
  return); Jira-deleted issues become `deleted_remote`. Sync-created issues
  carry their Jira identity in `extension_metadata`, closing the
  crash-between-create-and-link duplication window.
- **API** — `src/routes/jira_sync.rs` under `/v1` (session +
  project-membership gated): GET/PUT/DELETE
  `/projects/{id}/jira-sync`, POST `…/test`, POST `…/sync-now`. The
  credential is write-only: `has_credential` in responses, `credential:
  null` on update keeps the stored one.

### Frontend (`packages/web-core`, `packages/ui`)

- `JiraSyncSettingsSection` registered as a `jira-sync` section in the
  settings dialog: connection form (URL, auth mode, masked credential, JQL,
  interval, enable toggle), test-connection with match count, editable
  status-mapping tables, sync status (last run / running / error / link
  counts), sync-now and disconnect actions.
- `jiraSyncApi` + `useJiraSync` React Query hook.
- Board cards show a Jira key badge (`JiraBadge`) linking to the issue,
  dimmed when the link is dormant/deleted, fed by the
  `PROJECT_JIRA_LINKS_SHAPE` Electric shape through `ProjectProvider`.

## Verification

Unit tests cover the mapping/merge decision tables and client parsing. A
live E2E run (remote server + temp Postgres + mock Jira, single-user mode,
1 s tick) verified: import with mapped statuses, idempotent re-sync,
VK→Jira single-field PUT, echo-free follow-up passes, Jira→VK edits,
dormant/deleted_remote transitions, dormant reactivation without
duplicates, credential redaction, delete-config-keeps-issues, and
test-connection success/failure paths.

## Known v1 limits (by design)

- Synced fields are exactly title/description/status; VK-born issues are
  not pushed to Jira; one Jira connection per project.
- Conflict LWW uses issue-level `updated_at` on the VK side.
- Formatting fidelity between Jira wiki markup and VK markdown is
  best-effort (string pass-through).
