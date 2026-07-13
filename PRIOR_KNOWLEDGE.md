# Prior Knowledge — recalled for `vk/2f63-auto-archive-wor`

Searched the project knowledge base (`docs/knowledge-base/` — `INDEX.md` + topic
pages) for anything about issue status changes, workspace archiving, or the
remote issue-mutation path.

## Matches

- **`remote-external-integrations.md`** — closest hit. Confirms the `crates/remote`
  conventions this task rides on: mutations go through REST handlers that run
  their DB work inside a transaction and return a Postgres `txid`; the client
  waits on that `txid` over the ElectricSQL stream before dropping optimistic
  state. Any side-effect (like archiving) must happen **inside the same
  transaction** as the triggering write so it is covered by the returned `txid`.
  (Contributing task: `fec4-vk-slack-shortcu`.)

## No direct match

There is **no** existing KB page about issue-status → workspace archiving, the
`project_statuses` name-matching approach, or terminal-status handling. The KB
is otherwise about log normalization and MCP connectivity/OAuth — unrelated.

## What I relied on instead (from the code, not the KB)

- `crates/remote/AGENTS.md` — ElectricSQL read-path vs REST write-path; the
  txid handshake; "writes go through the REST API".
- Existing feature `archive_workspaces_for_done_issue` (commit `3510c588`) in
  `crates/remote/src/routes/issues.rs` — the exact pattern to generalise:
  status-change guard → name-match → list active → (Done-only) unmerged-PR warn →
  archive, all on the caller's `&mut PgConnection`.
- `db/project_statuses.rs::DEFAULT_STATUSES` — confirms `"Done"`/`"Cancelled"`
  are the built-in terminal status names matched by the hook.

## Reusable knowledge to capture on completion (stage 12)

A page on "terminal-status side effects on the remote issue-update path": the
status-change guard + `project_statuses` name-match idiom, why the side effect
must share the update transaction (txid), and the Done-vs-Cancelled warning
distinction. Not yet in the KB — worth adding.
