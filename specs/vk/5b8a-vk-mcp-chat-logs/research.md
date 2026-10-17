# Research: session chat history

## R1. Extend `/messages` or add a route?

**Chosen:** a new `/api/sessions/{id}/history` route.

`/messages` returns per-execution fields (`status`, `exit_code`,
`final_message`, `execution_id`). They would become ambiguous across many
turns, and its two MCP tools are documented as per-execution. A separate route
keeps both contracts honest.

## R2. Where do timestamps come from?

`NormalizedEntry.timestamp: Option<String>`. A grep of
`crates/executors/src` shows normalizers do not populate it in practice, so
entries are mostly `None`. `ExecutionProcess.started_at`/`completed_at`
are always present. Decision: effective time = entry time if it parses as
RFC 3339, else the turn's `started_at` (the same fallback the chat UI uses,
`specs/vk/a22f-time-stamp-chat` FR-3). In practice the time filter is turn
granular, and the tool description says so.

## R3. Keyword matching over truncated text

`project_messages` truncates to 4000 chars. Matching against the truncated
text would miss long tool outputs, which is exactly where a keyword search is
useful. Decision: match on the untruncated trimmed content, return truncated
`text` plus a `match_snippet` taken from the untruncated text.

Case folding uses `to_lowercase()` on both sides, matched as chars to keep
snippet boundaries on code points. Non-1:1 case mappings (e.g. `İ`) can shift
indices. The snippet is computed by searching a per-char lowercased vector
(each char lowercased to its first lowercase char), so indices stay aligned
with the original chars.

## R4. Concurrency

The knowledge base (`lazy-loading-normalized-conversation-history.md`) says
cold legacy turns contend for one global permit. Parallel loading would
queue anyway and could starve the UI's own history reads, so loading is
sequential.

## R5. Budget value

MCP clients commonly time tool calls out at about 60 s (Claude Code's default
MCP tool timeout is longer, but ChatGPT connectors are shorter). 20 s leaves
room for the MCP→backend hop and serialization. It is a constant, not
configurable (constitution III, no speculative generality).

## Alternatives rejected

- **Opaque cursor paging**: offset is enough because the result set is
  deterministic for settled turns. A running last turn can grow between
  calls, but only at the end in `asc` order, so earlier pages stay stable.
- **Workspace-wide search**: deferred (clarification 1).

## R6. Relation to global search (`routes/global_search.rs`)

Global search already finds "old turns", but it matches only
`coding_agent_turns.prompt`/`.summary` (the user prompt and final reply),
caps results at 20 snippets per category, and serves the UI dialog across hosts.
It cannot return intermediate assistant commentary or tool output and has no
MCP surface or time/turn filters. The new route reads the full normalized
projection for one session, so it complements global search rather than
duplicating it. The snippet helper is not shared: global search returns a
fixed 240-char prefix-biased window without clip markers, and changing it
would alter that UI's output.
