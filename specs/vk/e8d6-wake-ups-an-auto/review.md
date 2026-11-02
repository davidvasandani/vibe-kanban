# Independent review and validation

Seven independent Codex review passes completed (five initial passes, a full
post-integration review against origin/main, and a focused review of its fix).
Confirmed findings were addressed:

1. Generate ArchiveIntent's TypeScript export.
2. Revalidate withdrawn/replaced remote events, retaining replacement retry selection.
3. Admit eligible local merges during remote outages and attribute only admitted events.
4. Show pending provenance ahead of acknowledged history in active workspaces.
5. Forward paired origin names through Codex stdio MCP's restricted environment.
6. Preserve completion intent during activation, completion, and post-login sync.
7. Capture initially archived remote inserts.
8. Reload persisted workspace state during wake-up validation after poller preparation
   creates a worktree. The regression test retains the stale pre-creation snapshot.

The full post-integration review identified only item 8. The final focused review
confirmed the fix and reported no introduced defects.

## Completed verification

- `pnpm run format`, `pnpm run check`, and `pnpm run lint`: passed.
- Targeted Rust wake-up, workspace, lifecycle, origin forwarding, and poller suites:
  passed; this includes 15 workspace tests, 13 server wake-up tests, and 6 MCP
  poller/target tests.
- Frontend provenance/sidebar tests: 14 passed.
- Live PostgreSQL 16 migration validation: archived inserts, metadata stability,
  new event identity after rearchive, and transactional rollback passed.
- PR #388 CI on 81834221: frontend, backend Clippy/tests/schema, and remote tests passed.
- Focused server wake-up rerun for item 8 is in progress; PR checks must also pass
  on the final pushed commit before merge.

Knowledge pages and index are committed. The latest main was merged; conflicts
were confined to task-specific root specification/plan files, which retain this
feature. No other service or homelab deployment was changed.

A second integration pulled main 0493a8a7 (standalone dispatcher wiring and
pipeline-artifact isolation). Root task docs retain this feature and wiki
contributor entries retain both tasks. The local archive-intent migration was
renumbered to 20261102000000 to avoid the new base migration version. Final
verification and a full review are being repeated on this integrated tree.
