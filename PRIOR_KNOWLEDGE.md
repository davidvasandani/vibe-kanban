# Prior Knowledge: Lazy-loading workspace chat

The project knowledge base is populated. The relevant indexed pages are:

- `docs/knowledge-base/claude-log-normalization.md`
- `docs/knowledge-base/collapsing-repeated-log-entries.md`

The recent task `a9622cfd` (cancel abandoned historical log replays) is also
directly relevant implementation history, although it has not yet been
distilled into an indexed topic page.

## Distilled guidance

1. Normalized conversation transport is a JSON Patch stream over a virtual
   `/entries/{index}` array. `add` creates a row and `replace` refines an
   existing row, so pagination cannot safely treat raw patch frames as
   independent messages without preserving process-local index semantics.
2. Entry indexes are allocated across all stdout/stderr producers for one
   execution process by a shared `EntryIndexProvider`. A cursor or stable key
   must therefore retain the execution-process boundary and absolute normalized
   entry identity.
3. Historical replay can be large enough to exhaust memory. Existing repeated
   event compaction bounds individual replacement payloads, but it does not
   bound the number of distinct entries replayed or retained by the client.
4. Normalizers may emit many replacements for one visible index. Page-boundary
   tests must include replacements, not only append-only transcripts, and the
   final paged state must match full replay.
5. `AmpResume` can remove replayed entries and reset indexes inside a process.
   Pagination must operate on the durable final normalized view (or include
   reset operations in its materialization) rather than assuming indexes only
   increase for the lifetime of raw logs.
6. Tool-call lifecycle updates can arrive late or out of order. A live handoff
   must not allow an old replacement to overwrite a newer row, and must not
   lose events between the tail snapshot and live subscription.
7. The server already cancels normalized historical replay if the browser
   closes the WebSocket while `stream_normalized_logs` is still being built.
   Any replacement paged API must preserve cancellation: abandoned scroll or
   scope-change requests must stop backend normalization promptly.
8. Server-side normalization is shared by local, remote, desktop, and mobile
   consumers. The durable pagination contract belongs beside that shared
   transport; frontend-only slicing would reduce DOM work but not backend work,
   transfer, or retained source state.

## Implications for the spec and plan

- Define pages in terms of the materialized normalized entry state, with stable
  process-local identities, rather than arbitrary JSON Patch frames.
- Keep live patch streaming as an incremental channel, but make its snapshot
  boundary explicit so a bounded tail can transition to streaming safely.
- Preserve request cancellation and test it alongside page correctness.
- Treat cursor validation, reset behavior, and replacement patches as core
  contract cases rather than later hardening.
- Do not change homelab deployment configuration; this is an application
  transport/state-management change inside the Vibe Kanban repository.
