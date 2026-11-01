# Data model and contracts

Archive intent needs workspace ID, stable trigger key, rule, trigger time, acknowledgement and most recent archive time. Pending delivery state remains in issue_comment_deliveries. Explicit unarchive acknowledges completion events; scheduling only temporarily holds them.

schedule_wakeup keeps its existing request shape. Within a launched agent, workspace/session may be omitted. Invalid target or removed worktree returns an actionable error; valid archived targets reactivate atomically with timer acceptance. Archive provenance is additive API data, retaining legacy unknown origin explicitly.
