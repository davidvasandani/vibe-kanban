# Prior knowledge: scheduled wakeup pending

Read-only search of vibe-kanban/wiki and docs/knowledge-base found:

- wiki/durable-session-wakeups.md: timers are durable local metadata plus delivery ledger rows with nil issue IDs. A five-second sweep must reconsider overdue timers; remote issue access is intentionally unnecessary. Cancellation and claim are atomic. Receipt reads must be bounded.
- wiki/issue-comment-delivery.md: the dispatcher requires a durable turn boundary, no active session turn, no queued user message and no blocking workspace process. Restart reconciliation substitutes for lost in-memory boundaries. An execution row is not launch proof; worker acceptance, pgid or agent session evidence is required.

Preserve these safeguards. The affected timer is pending with attempt_count=0, no reason/error and next_attempt_at=2026-11-01T00:15:00Z. Inspect sweep selection before changing retry or active-turn policy. Do not replay the stored operational prompt during diagnosis.
