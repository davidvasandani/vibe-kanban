# Prior Knowledge: Codex/ChatGPT refresh-token race (VAS-490)

Searched the project knowledge base (`wiki/`) and its `INDEX.md` for anything
about Codex auth, credentials, tokens, concurrency, and the scoped/worktree
home layout.

## No existing page covers Codex credentials

There is **no** knowledge-base page about Codex/OpenAI authentication,
`auth.json`, token refresh, or credential sharing across executions. This task
will create the first one (see stage 12). The pages below are the closest
adjacent context.

## Relevant matches

1. **`agent-process-lifecycle.md`** — the strongest match.
   - "One turn = one `ExecutionProcess` row = (today) one OS process lifetime."
     So each concurrent task attempt / follow-up / review is its own
     `codex app-server` OS process. That is the concurrency source that makes
     several processes hit the shared `auth.json` at once.
   - Codex is a **stdio JSON-RPC** app-server whose turn end is coupled to the
     reader-loop teardown; it is deliberately **not** kept warm (Phase 3
     deferred). So we cannot assume a single long-lived Codex process funnels
     all refreshes — each turn spawns anew and re-reads credentials.
   - The lifecycle doc is full of "no cross-actor lock ⇒ race" gotchas
     (insert-before-remove, generation-conditional reap). Same failure family
     as this bug: a shared resource (`auth.json`) mutated by concurrent actors
     with no cross-process serialization. The fix should add exactly that
     serialization for the auth handshake.

2. **`managed-cli-tool-catalog.md`** — "host-first PATH propagation across local
   and clustered workspace process boundaries." Confirms the two deployment
   shapes (local in-process vs clustered worker processes) that any
   cross-process mechanism must cover. Our lock must work for both — an
   in-process async mutex is insufficient alone; a file lock is required.

3. **`self-hosted-deployment.md`** — deploy/hosting lives in the homelab repo
   (`modules/vibe-kanban-rebuild.nix`); cross-repo sequencing is guarded. Not
   directly needed for this code change, but confirms deployment is external.

## How this shapes the plan

- Serialize the refresh **across processes** (file lock), not just in-process,
  because clustered deployment runs executions as separate worker processes.
- Do it in the brief startup handshake, never across a turn — the lifecycle doc
  shows turns are the expensive, concurrency-critical unit.
- The shared `auth.json` is a single inode (same file locally, symlink target
  in the worker scoped home), so one lock coordinates all actors.
- Record the new credential knowledge as a fresh KB page at the end.
