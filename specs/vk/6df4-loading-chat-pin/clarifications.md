# Clarifications: Resource-Aware Chat Loading

Task: `vk/6df4-loading-chat-pin`

## Resolved decisions

### Last-reader cancellation

Cancel historical reconstruction when its final reader disconnects. The user
reported interactive CPU starvation, so speculative warming after nobody needs
the result would optimize a possible future read at the expense of current
work. A remaining owner or joined reader keeps the single shared operation
alive. Completion still writes the durable atomic sidecar, so any operation
that reaches completion benefits every later reader.

### Coordination scope

Use process-local single-flight in this increment. The deployed local workspace
chat is served by one Vibe Kanban coordinator process, and the current
historical-normalization semaphore is already process-local. The atomic sidecar
remains the cross-process/restart truth, and the leader rechecks it after
acquiring ownership. Cross-process exclusion would require a crash-safe file or
database lease and recovery policy that the observed deployment does not need.

This decision is not permission to run duplicate server instances against the
same data directory. If the deployment model changes, cross-process
materialization coordination must be specified before doing so.

### Cross-node escalation gate

Do not add distributed reconstruction now. First ship avoidance, single-flight,
bounded concurrency, and measurements. Reconsider an explicit cross-node
protocol only if representative cold-cache histories still exceed both of these
operational targets after the change:

- cold-cache time to first usable completed-history result exceeds 2 seconds at
  p95; and
- reconstruction holds the serving node above one fully utilized core for more
  than 2 seconds at p95, or measurably delays live chat/execution requests.

These are investigation gates rather than promises that any idle worker is
eligible. A later design must authenticate the job, prove shared-source access,
bind ownership/retry identity, and preserve affinity semantics.

## Remaining open questions

None.
