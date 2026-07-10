# Implementation Plan: Bidirectional Jira ↔ VK Sync (task vk/d2aa-sync-vk-and-jira)

Step-by-step build order. The authoritative dependency-ordered task list is
`homelab/specs/vk/d2aa-sync-vk-and-jira/tasks.md` (T001–T017); this is the
executable narrative. Rationale in `SPEC.md`; prior-art recall in
`PRIOR_KNOWLEDGE.md`.

## Step 1 — Schema (T001)

`crates/remote/migrations/20260709000000_jira_sync.sql`: create
`project_jira_configs` (unique per project, encrypted credential, JQL,
interval, JSONB status mapping, sync stamps, `created_by_user_id`) and
`jira_issue_links` (unique per `(project_id, jira_issue_id)` and per
`issue_id`; link_state; last-synced snapshot columns; `last_error`);
`set_updated_at` triggers; electrify `jira_issue_links`.

## Step 2 — Jira domain module (T002, T004–T006)

- `crates/remote/src/jira/types.rs` — auth-mode/config/link/request/response
  types (`ts_rs::TS` derives).
- `crates/remote/src/jira/client.rs` — reqwest client over the shared
  `AppState.http_client`; Cloud `/rest/api/2/search/jql` vs Server
  `/rest/api/2/search` pagination; issue GET/PUT; transitions; myself;
  approximate-count; credential-free error mapping. Unit tests for datetime
  and error-body parsing.
- `crates/remote/src/jira/mapping.rs` — override → category-default
  resolution, explicit reverse table, seeding. Unit-tested.
- `crates/remote/src/jira/merge.rs` — per-field 3-way decision
  (`NoOp`/`WriteVk`/`WriteJira`, LWW conflict, Jira wins ties). Unit-tested.

## Step 3 — DB repository (T003)

`crates/remote/src/db/jira_sync.rs` — config CRUD (upsert keeps stored
credential via `COALESCE`), due-config scan (level-triggered: interval
elapsed OR `sync_requested_at` newer than last start), pass stamps, link
CRUD + snapshot update + counts, orphan-issue lookup by
`extension_metadata #>> '{jira,issue_id}'`, `next_sort_order`.

## Step 4 — Reconciler (T007–T008)

`crates/remote/src/jira/sync.rs` — 30 s ticker (`JIRA_SYNC_TICK_SECS`); per
config: search → seed mapping → per-issue import/3-way sync (VK writes +
snapshot in one transaction; Jira re-read after outbound writes) →
scope-out detection (dormant/deleted_remote) → aggregate stamps/errors.
Spawned from `crates/remote/src/app.rs` beside `spawn_cleanup_task`.

## Step 5 — API + shape (T009–T011)

- `crates/remote/src/routes/jira_sync.rs`: GET/PUT/DELETE
  `/v1/projects/{id}/jira-sync`, POST `…/test`, POST `…/sync-now`; all
  `ensure_project_access`-gated; credential write-only. Merged in
  `routes/mod.rs` `v1_protected`.
- `PROJECT_JIRA_LINKS_SHAPE` in `shapes.rs` + fallback route/handler in
  `shape_routes.rs` (required by the hybrid-sync contract — see
  PRIOR_KNOWLEDGE.md).
- Register types in `src/bin/generate_types.rs`; run
  `pnpm run remote:generate-types` and `pnpm run remote:prepare-db`
  (note: environment lacks a standalone `sqlx` binary — shim it to
  `cargo sqlx` for `scripts/prepare-db.sh`).

## Step 6 — Frontend (T012–T014)

- `jiraSyncApi` in `packages/web-core/src/shared/lib/api.ts`
  (`makeRemoteRequest`), hook `useJiraSync.ts`.
- `JiraSyncSettingsSection.tsx` + `jira-sync` registration in
  `settingsRegistry.tsx` (org/project picker, connection form with masked
  credential, test-connection, mapping editors, status block,
  sync-now/disconnect).
- Badge: subscribe the shape in `ProjectProvider.tsx`
  (+ `getJiraLinkForIssue` in `useProjectContext.ts`), new
  `packages/ui/src/components/JiraBadge.tsx`, `jiraLink` prop on
  `KanbanCardContent.tsx`, wired in `KanbanContainer.tsx`.

## Step 7 — Gates + verification (T015–T017)

All typechecks/lints/tests/format + generated-artifact checks, then a live
E2E: temp Postgres (initdb) + mock Jira (python) + `remote` binary in
single-user mode with 1 s tick, driving the acceptance criteria end-to-end
(import, idempotency, both sync directions, echo-freedom, dormancy,
deletion semantics, credential redaction, auth gating).
