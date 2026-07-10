# Prior Knowledge — recalled for `vk/d2aa-sync-vk-and-jira`

Searched both project knowledge bases — `wiki/` (primary, 5 pages) and
`docs/knowledge-base/` (2 pages) — for pages relevant to this task
(bidirectional Jira sync: remote-server background service, new Electric
shape, project-settings UI, external REST client, credential storage).

## Relevant findings

**[wiki/electric-sync-fallback.md] — directly relevant.** Any new Electric
shape must participate in the client's hybrid sync: Electric-first with a
readiness timeout, then a locked REST fallback polling the shape's
`fallbackUrl` (30 s snapshots). Practical consequence for this task: the new
`PROJECT_JIRA_LINKS_SHAPE` is not complete without a registered REST fallback
route + handler in `shape_routes.rs` — otherwise boards running in fallback
mode (Electric down/unreachable) would silently never show Jira badges.
Applied: `/v1/fallback/jira_links` handler registered alongside the shape.

**[wiki/self-hosted-deployment.md] — context for rollout.** The remote server
binary (`remote`) ships via the versioned-release contract
(`VK_RELEASES_DIR`, atomic `current` flip, one-step rollback). SQLx
migrations run at server startup (`app.rs` → `db::migrate`), so the
`jira_sync` migration applies on first boot of the new release; rollback to
`previous` leaves the two new tables in place unused, which is harmless —
consistent with "sync never deletes VK issues".

**[wiki/kanban-items-state-and-activity-grouping.md] — convention echo.**
Board semantics identify the "In progress" column by *name*, not id. The
Jira status mapping follows the same convention (name-keyed mapping tables,
case-insensitive resolution against `project_statuses.name`), so renamed
columns degrade the same way in both features rather than inventing a second
identity scheme.

**Not relevant:** `wiki/mobile-kanban-scrolling.md` (touch gestures),
`wiki/project-context-map.md` (issue scoping in monorepos),
`docs/knowledge-base/claude-log-normalization.md` and
`collapsing-repeated-log-entries.md` (executor log processing).

## Gaps the knowledge base did not cover

No prior page covered: remote-server background reconcilers, external
connector credential storage (found via code precedent:
`organization_env_vars` + `JwtService::encrypt_string`), or external REST
client patterns (`github_app/service.rs`). These are candidates for new
pages when this task distills its knowledge.
