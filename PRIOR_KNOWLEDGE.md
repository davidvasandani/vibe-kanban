# Prior knowledge: session execution exclusivity

Read-only search of vibe-kanban/wiki found relevant lifecycle, worker hardening, issue comment delivery, and durable wake-up pages.

- agent-process-lifecycle.md: execution IDs own children and exit monitors. Turn liveness differs from process liveness. Cleanup and queue handoffs occur after DB completion. Warm reuse must preserve ownership across handoff and teardown.
- issue-comment-delivery.md explicitly records that user follow-ups are not serialized with dispatcher admission or other follow-ups. Durable session_turn_boundaries prevent comment dispatch during finalization; checking only Running rows misses queue handoff gaps.
- durable-session-wakeups.md: timers reuse comment dispatcher session locks and durable delivery records, with user-queue priority. Preserve their existing admission and launch-evidence semantics.
- worker-execution-hardening.md: worker execution and coordinator reconciliation are separate authorities. Inspect current guards before adding another lifecycle owner.

Implications: centralize admission across callers, reserve before asynchronous startup, and independently guard worker process lifetime. Do not use a startup grace period, infer idle from an empty queue, or release a process guard merely because a DB row becomes terminal. Keep warm-server reuse and automatic continuations compatible.

Source task: vk/0890-investigate-a-fo (workspace 089057cb-ac81-44f3-969e-4aafdbfbdfb4).
