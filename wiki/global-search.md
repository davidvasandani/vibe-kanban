# Global search across remote metadata and host conversations

The searchable data has two authorities. Remote PostgreSQL owns organizations,
projects and cloud workspace metadata; local SQLite owns workspace branches,
sessions and persisted coding-agent turns. Search the remote membership/owner
scope and each authorized host independently. The selected organization, active
host and currently mounted Electric collections cannot define global coverage.

## Transport and partial failure

The local shell targets other hosts with explicit `/api/host/{id}` scoping. The
remote shell must keep the unprefixed `/api/global-search` path and select its
relay host separately; applying both forms sends a host proxy path through the
wrong relay. Preserve host identity on every result for navigation.

Start local and remote metadata requests before awaiting host discovery. A
failed directory must not consume the entire budget before healthy local work
starts. Race transport promises against the search deadline: passing AbortSignal
alone is insufficient because WebRTC may keep its own longer request timeout.
Stop scheduling host batches at the deadline and report unsearched sources.

## Conversation storage and bounded work

`coding_agent_turns.prompt` and `.summary` provide historical user prompts and
final assistant replies without replaying execution logs. They do not contain
intermediate commentary or tool output; state that scope in the UI. Exclude
dropped execution processes and retain archived workspaces.

SQLite `lower()` folds ASCII only. Local search streams candidates and folds
Unicode in Rust, then retains at most 20 snippets per category. Snippet offsets
must map folded bytes back to original characters. A SQLite progress callback
bounds database work; an async deadline and cooperative yields bound the reader.
A connection carrying a request-specific callback must be closed on drop so
cancellation cannot return that callback to an unrelated pooled request.
Remote PostgreSQL uses membership joins and a transaction-local statement timeout.

## Navigation coordination

The local shell normally redirects to the first project after an organization
change. An explicit search selection must update the shell's previous-org marker
before changing the selected organization; otherwise a later shape response
replaces the user's destination.

Chat results use `searchSessionId` to select the historical session after it
loads. Preserve valid session selection on refetch, and clear search intent on
manual session changes so selecting the same result again reapplies it. Combine
cloud metadata with host workspace matches; when the host is unavailable, label
the cloud result as opening its linked issue/project context.

## Verification

`globalSearch.test.ts` covers aggregation, routing, duplicates and deadlines;
`GlobalSearchDialog.test.tsx` covers stale results and shell coordination;
`useWorkspaceSessions.search.test.tsx` covers historical/repeated selection.
Local Rust tests exercise matching and category bounds. Run
`python3 scripts/test-global-search-postgres.py` with PostgreSQL binaries on PATH
to validate the actual remote SQL in an isolated temporary database.

## Contributed by

- vk/8f5e-global-search
