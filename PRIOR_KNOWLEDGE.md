# Prior knowledge: VK MCP session chat history

Task `vk/5b8a-vk-mcp-chat-logs`. Distilled from `wiki/`, `docs/knowledge-base/`,
`crates/mcp/AGENTS.md` and earlier task specs. Read-only summary.

## Existing message readers (`vk/29d8-vk-list-all-mess`, `crates/mcp/AGENTS.md`)

- `list_recent_messages` / `list_all_messages` / `get_execution.final_message`
  are projections of `ContainerService::normalized_entries` served through
  `GET /api/execution-processes/{id}/messages` and
  `GET /api/sessions/{id}/messages`. The session route resolves **only the
  newest non-dropped `CodingAgent` execution**. That is the gap this task
  closes.
- Projection rules to reuse rather than fork: `entry_role` (drops `Loading`
  and `TokenUsageInfo`), empty-text skip, 4000-char truncation with a
  `… [truncated]` marker, ids `{execution_id}:{entry_index}`, `parse_roles`.
- 29d8 clarified that `all` means "the settled normalized projection Vibe
  Kanban has", including the legacy normalizer's newest-2,000-raw-message
  safety window and its omission notice. It is not an unbounded raw-log replay.
  The same caveat applies per turn here.
- Execution-first precedence when both `session_id` and `execution_id` are
  given.

## Cost of reading turns (`docs/knowledge-base/lazy-loading-normalized-conversation-history.md`, `wiki/awaited-stream-settlement.md`)

- A finished turn with a materialized sidecar is a cheap replay. A legacy
  turn without one needs a full vendor re-normalization, gated by a per-execution
  lease plus a **global historical-normalization permit**. A session-wide
  read should therefore load turns **sequentially**, and should skip turns
  it can rule out without reading (time-window pruning), not fan out.
- Running turns are read from the live in-memory `MsgStore` snapshot.

## Timestamps (`specs/vk/a22f-time-stamp-chat`)

- `NormalizedEntry.timestamp` is `Option<String>` and most normalizers leave
  it `None`. The chat UI falls back to the execution record's time when an
  entry has none, and never fabricates "now". Date filters should do the same
  (entry timestamp, else turn `started_at`), so in practice time filtering is
  turn-granular for most executors.

## MCP conventions (`crates/mcp/AGENTS.md`, `wiki/mcp-pr-tools-and-connection-notices.md`)

- Every session-reading tool fetches the session and calls
  `scope_allows_workspace` before reading.
- Tools are registered by name in `tools/mod.rs` (the scoped tool list) as well
  as through the `#[tool_router]` impl.
- The MCP talks HTTP to the backend and mirrors response structs locally
  (e.g. `RecentMessagesPayload`) instead of importing server types.

## Constitution points that apply

- III/VI: extend the existing `/messages` projection; no new store.
- II: unit-test the pure filter, paging and time logic with `#[cfg(test)]`.
- IX: unknown or absent timestamps degrade safely.
