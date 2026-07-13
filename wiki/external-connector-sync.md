# External-Connector Sync (remote server): reconciler, credentials, 3-way merge

How to connect the remote server to an outside system (shipped for Jira in
`crates/remote/src/jira/`; the shape is connector-agnostic). Covers the
pieces that were non-obvious or that an independent review had to force.

## Where such a feature lives

The board users see is the **remote** stack (`crates/remote`: Axum +
Postgres + ElectricSQL) — issues are `Issue` rows streamed via shapes, and
"Project Settings" is the `settingsRegistry.tsx` dialog in `web-core`. A
connector implemented against the local SQLite `tasks` model would sync a
table nobody looks at. Check this first; it decided the entire architecture
of task d2aa.

## Credential storage & the destination-pinning rule

- Store secrets as `state.jwt.encrypt_string(...)` ciphertext (AES-256-GCM,
  same as `organization_env_vars`); responses carry `has_credential: bool`,
  never the secret; `credential: null` on update means "keep".
- **The stored credential may only ever be sent to the stored destination.**
  Any endpoint that can combine a *stored* secret with a *caller-supplied*
  URL is an exfiltration primitive (attacker points the URL at their server
  and reads the Authorization header). This bit us twice: the test-connection
  endpoint, then the PUT path (save new URL + `credential: null`, wait for
  the next sync pass). Rule: changing base URL or auth mode requires
  re-entering the credential — and the check must be **atomic with the
  write** (Jira: the upsert's `ON CONFLICT DO UPDATE ... WHERE` clause), not
  a route-level pre-check, or concurrent saves reopen it (TOCTOU).
- Credential-bearing routes (save/delete/test) gate on **org admin**
  (`assert_admin`), mirroring `organization_env_vars`; read/status routes are
  member-level. Outbound requests to an admin-supplied URL are inherent to a
  connector (self-hosted Jira lives on private networks) — the mitigation is
  who can set the URL, not where it may point.

## Reconciler pattern (single-writer background loop)

`spawn_*_task(pool, http_client, jwt)` from `app.rs`, `tokio::time::interval`
(pattern: `attachments/cleanup.rs`). Level-triggered throughout:

- "Sync now" is a timestamp flag (`sync_requested_at`), not a Notify — a
  request during a running pass isn't lost, and a crashed pass retries on
  the next tick because `last_sync_completed_at` stays stale.
- **Pass claiming must be a real lease** (independent review round 2–3):
  `last_sync_started_at > last_sync_completed_at` = lease held; the
  authoritative claim is one atomic `UPDATE ... WHERE enabled AND <due> AND
  <lease free> RETURNING *` — the pass runs from the returned row, so a
  disable/edit racing the tick takes effect immediately. Completion is
  guarded by the lease token (`WHERE last_sync_started_at = $claimed`), so a
  zombie pass that outlived a stale-lease takeover can't free the new
  holder's lease. Stale takeover (60 min) is a crashed-pass backstop only.
- Per-item failures never abort a pass; record on the item row
  (`last_error`) + aggregate into the config (`last_sync_error`) for the UI.

## Echo-free bidirectional sync: per-field 3-way merge

Store a **last-synced snapshot** of every synced field on the link row and
compare each side against it (`jira/merge.rs`): only-remote-moved → write
local; only-local-moved → write remote; both → LWW with the remote system
winning ties. Key properties:

- Snapshot updates happen in the **same transaction** as the local write —
  the reconciler's own writes can never look like user edits.
- After writing to the remote system, **re-read it** and snapshot the
  returned representation; remote-side normalization otherwise looks like a
  fresh remote edit next pass.
- Idempotent import: unique on `(project_id, <remote immutable id>)` (keys
  can be renamed; internal ids can't), plus the created issue carries its
  remote identity in `extension_metadata` so a crash between issue-create
  and link-create is re-linked, not duplicated (issue+link can't share a tx
  because `IssueRepository::create` owns its own).
- Pagination truncation must be explicit: a capped search result must not
  drive "item left the query scope" logic, or big result sets mass-dormant
  live links.

## Deletion semantics that survived review

Links FK → the **config** row (`ON DELETE CASCADE`), not just the project:
disconnect deletes links atomically and an in-flight pass inserting after
the delete hits an FK violation instead of resurrecting rows. Sync never
deletes VK issues; a remote-deleted issue just permanently unlinks
(`deleted_remote`), and out-of-JQL-scope links go `dormant` and resume on
the same VK issue if they return.

## Misc gotchas

- New Electric shape ⇒ fallback REST route + handler in `shape_routes.rs` is
  mandatory, or fallback-mode boards silently miss the data (see
  `electric-sync-fallback.md`).
- `define_shape!` compile-time-validates its SQL against `.sqlx/` metadata —
  a new shape's table must exist in a fresh `remote:prepare-db` run first.
- `scripts/prepare-db.sh` calls a standalone `sqlx` binary; on hosts with
  only `cargo sqlx`, shim it (`printf '#!/usr/bin/env bash\nexec cargo sqlx "$@"' > /tmp/sqlx-shim/sqlx`).
- Jira API: Cloud removed legacy `/rest/api/2/search` (2025) — use
  `/search/jql` + `nextPageToken` on Cloud, classic `startAt`/`total` on
  Server/DC; API **v2** shapes keep rich text as strings (no ADF). Status
  changes only via the transitions API. Jira timestamps are
  `%Y-%m-%dT%H:%M:%S%.f%z` (offset without colon — not RFC 3339).
- Full E2E is cheap without Docker: `initdb` temp cluster +
  `VIBEKANBAN_SINGLE_USER_MODE=1` (POST `/v1/auth/single-user/login` returns
  a bearer token) + a python mock of the external API + a 1 s tick env var.

## Surfacing the link in the UI

The connector link is streamed to the client on the link row
(`jira_browse_url` + `link_state`) via `PROJECT_JIRA_LINKS_SHAPE` and read with
`getJiraLinkForIssue(issueId)` from `useProjectContext()`. It surfaces in two
places that share one `JiraBadge` component, one `jiraLink` data prop, and that
one lookup — the kanban card and the issue detail panel header. Adding the link
to a new UI surface is therefore pure presentational wiring (no
backend/schema/type change). Details + placement/gating rules:
[kanban-issue-panel-sections.md](kanban-issue-panel-sections.md).

## Contributed by

- `vk/d2aa-sync-vk-and-jira`
- `vk/a793-vk-jira-bi-direc`
