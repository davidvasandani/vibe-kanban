# Technical specification: Global Search

Provide one globally accessible search experience across organizations, projects,
workspaces, and persisted chat content available to the current user. Results must
identify their category and context and navigate to the corresponding entity or
conversation. Search must not depend on the currently selected organization or
project and must preserve existing authorization boundaries.

Use the existing application shell and dialog conventions. Support keyboard and
pointer use, debounced queries, loading, empty, error and partial-failure states.
Match names and chat text case-insensitively using literal search terms. Do not
search tool output, credentials, or raw execution logs as conversational messages.
Bound server work and response sizes; do not download all chat transcripts into
the browser. Empty queries perform no expensive search. Stale responses must not
replace newer results. Include archived workspaces where accessible, and label
context clearly. No other service or deployment changes are in scope.

Inspect existing local/remote ownership before selecting endpoint locations.
Reuse existing identity and navigation mechanisms. Add regression coverage for
matching, authorization, result bounds and navigation as applicable; run repository
format, checks and lint, followed by independent Codex review. Document limitations
and reusable knowledge before opening and merging the pull request.

## Implemented contract

Global Search is available in both app rails, mobile navigation drawers, and
Ctrl/Cmd+Shift+F. Queries are debounced 250 ms and accept 2–200 characters.
`/api/global-search` searches workspace names/branches and persisted chat prompts
and final replies using Unicode matching. `/v1/global-search` searches remote
metadata using membership and workspace-owner filters. Results are capped at 20
per category per source; database work has a three-second deadline and frontend
aggregation a twelve-second deadline independent of transport cancellation.

Search spans authorized online hosts and reports unavailable sources. Direct
workspace hits retain host/session navigation and cloud parent context; cloud-only
workspace hits explicitly open linked issue/project context. Raw execution logs,
intermediate assistant commentary and tool output are outside chat coverage.
