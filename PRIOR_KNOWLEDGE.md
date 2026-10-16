# Prior knowledge — vk/77a2-session-step-tim

Read-only search of `vibe-kanban/wiki` and `vibe-kanban/docs/knowledge-base` for timestamps and normalized logs found:

- `docs/knowledge-base/conversation-timestamp-provenance.md`: the shared ConversationTimestamp boundary prefers entry time but falls back to process creation time. Most normalizers omit timestamps; this explains repeated start times. Never stamp replay/render time. Preserve metadata through aggregation and hide timestamps for invisible entries.
- `wiki/session-chat-history-search.md`: normalized timestamps are mostly missing; session search explicitly separates event and turn time. Ordering must remain turn/index, not timestamp.
- `wiki/awaited-stream-settlement.md` and `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`: completed history uses materialized normalized sidecars, while active entries stream patches. Any fix must behave consistently across both paths and avoid inventing times when replaying historical logs.

These findings refine SPEC.md: investigate durable capture-time metadata at the shared normalization/persistence boundary; make legacy unknown event time honest.
