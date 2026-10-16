# Technical plan

Apply `/speckit.plan` using this feature's spec and the Vibe Kanban constitution.

1. Add an opt-in live constructor to `crates/utils/src/msg_store.rs`. Under the existing publish lock, enrich indexed normalized-entry add/replace patches with first server observation time, retaining timestamps by entry path. Source timestamps win; removal clears identity. Default stores remain replay-safe and never consult the clock. Bound timestamp retention; when exhausted, leave new unknown times unset.
2. In `crates/local-deployment/src/container.rs`, opt in only fresh child, fresh worker, and fresh detached-file tracking. Reattachment and historical stores use the default constructor. Fresh detached tracking is distinguished explicitly from adoption.
3. In `packages/web-core/src/features/workspace-chat/ui/ConversationTimestamp.tsx`, remove execution-start fallback. Synthetic prompt/script entries already supply explicit time. Keep grouping, hidden-entry behavior and formatting unchanged.
4. Test enrichment, replacements, removals, source priority, replay and serialization, plus rendered live update/reload and legacy UI behavior. Existing finished-execution sidecar materialization persists enriched patch values without new storage schema.

Constitution: II tested behavior; III shared minimal mechanism; IX unchanged vendor protocols; XI no invented historical evidence; XIX bounded metadata. Both shared frontend consumers affected. No hosting or other-service changes.

Risk: observation time is normalization time, so delayed worker delivery can differ from vendor occurrence time. After an in-flight restart or failed sidecar write, missing event times remain unknown. Do not claim recoverability for timestamps never durably recorded.
