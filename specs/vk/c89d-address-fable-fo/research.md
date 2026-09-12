# Investigation evidence

- 2026-09-12 01:27:36 and 01:29:23 UTC, think2 vibe-kanban-dev: f665b4c8-5872-4602-8063-815b31a079b5 and 72b2435a-1bd8-4b9e-a5b6-a150fdfea94b failed because repository cdce12c2-a050-49b1-86c3-3b28ace8ada8 administration lock was held by another owner.
- 2026-09-11 16:12:18 and 18:07:13 UTC: a642b27f-c094-4041-b9e0-706fd655108a and d2917abb-64a6-4d7b-8f96-cd7689a1bd6b failed at final provisioning state persistence.
- Older CURSOR_AGENT failures reflect scheduler capability rejection, distinct from intermittent provisioning.
- Repository commands point to specs/vk/c89d-address-fable-fo; use their exact requested paths while identifying the current task in the artifacts.
- No external API/library design changes or new dependencies planned.

## Confirmed correction

The admin mutex is keyed by canonical filesystem path while repository_admin_locks is keyed by repo UUID. A coordinator-local checkout and its shared bare store have different canonical common directories but the same registered repo ID. The original manager returns RepositoryLockBusy immediately when the SQLite lease belongs to another operation; an abandoned guard also retains its lease until expiry. Tests exercise both conditions without Git side effects.

Use the registered repository ID for the admin queue, matching the durable authority. For residual durable contention, poll only atomic acquisition at 100 ms intervals for at most the configured lease duration (plus any in-flight database call). Recompute wall-clock lease times on each attempt; use monotonic time for the wait budget. Preserve generation and operation-id predicates, retain tombstones, and never force release or retry Git/agent startup. Other repository IDs retain independent queues. No Drop-based release was added: blocking Git may outlive a cancelled future.

## Placement investigation limitation

Read-only inspection of the active database (db.v2.sqlite, confirmed through coordinator file descriptors) on 2026-09-12 found none of the four failed workspace rows: they have been removed. No trigger mutates placement state. In current source, the reported final-state error follows a compare-and-set that affects zero rows, not a SQL error. Deletion or a concurrent placement mutation can produce it; the retained evidence does not establish which happened to those removed workspaces. Do not weaken that ownership check or replay successful provisioning. No placement code change is justified by the available evidence.

## Hosting

Coordinator is think2, workers include think3/think5. The relevant repo is Vibe Kanban (cdce12c2-a050-49b1-86c3-3b28ace8ada8), registered at /srv/src/vibe-kanban. No hosting changes required.
