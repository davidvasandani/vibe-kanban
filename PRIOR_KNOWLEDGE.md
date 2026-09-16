# Prior knowledge — vk/1d23-vk-worker-output

Read-only recall searched Vibe Kanban's docs/knowledge-base index and topic pages, plus homelab/docs/knowledge-base and homelab/knowledge-base, for replay, indeterminate, and worker output. The knowledge base is populated.

- `vibe-kanban/docs/knowledge-base/clustered-workspace-execution.md`: worker event journals are bounded and monotonic; the coordinator acknowledges persisted sequences and reconnects from the acknowledged cursor. Reject real gaps; never invent completion or restart agents after disconnect.
- `vibe-kanban/docs/knowledge-base/authoritative-snapshot-stream-handoffs.md`: a final assistant response is not terminal evidence. Persist authoritative terminal state before acknowledging worker events. Retry captured terminal evidence without dropping it. UI activity follows persisted execution snapshots.
- `vibe-kanban/docs/knowledge-base/soft-restart-worker-drain.md`: workers own running children and journals across coordinator replacement. Coordinator startup must reconcile before orphan cleanup. A quarantined job can still own a live child; protocol state alone does not prove exit.

Implications: inspect cursor restoration versus retention after coordinator restart, distinguish durable transcript reconstruction from worker polling offsets, and preserve truthful indeterminate behavior for actual data loss. No relevant hosting change is established yet. Stage 1's preliminary spec precedes this recall as explicitly required; subsequent specification and plans build on it.

Additional recall after the user supplied stale Codex rollout paths:
- `vibe-kanban/docs/knowledge-base/cluster-mcp-runtime-connectivity.md`: execution-scoped Codex homes are deleted on teardown and the MCP root is cleared on worker startup; `sessions` is linked to persistent storage before launch. Persistent files do not make an absolute path through the disposable alias persistent.
- Local pinned Codex source (`44918ea`, Cargo checkout): rollout lookup validates a database path then falls back to filesystem discovery; `sqlite_home` config takes precedence over its environment fallback and otherwise defaults to `CODEX_HOME`. This provides an execution-local index boundary without deleting persistent transcripts.
