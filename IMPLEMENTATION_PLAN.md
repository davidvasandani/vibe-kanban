# Implementation plan: vk/ad2a-failed-to-start

1. **Liveness classifier** (`crates/services/src/services/cluster/scheduler.rs`):
   add a pure `dispatch_liveness(worker: Option<&WorkerNode>, now) -> Result<(), WorkerUnavailable>`.
   It returns unavailable when the row is missing, the status is `offline`, or
   the lease is missing or expired. A `draining` worker with a valid lease stays
   available. The result carries the hostname and last heartbeat so the caller
   can build the message. Unit-test every row of the SPEC table.
2. **Typed error** (`crates/services/src/services/container.rs`): add
   `ContainerError::WorkerUnavailable(String)`, which displays its message
   verbatim.
3. **Dispatch gate** (`crates/local-deployment/src/container.rs::dispatch_execution`):
   after resolving `worker_node_id`, load `WorkerNode::find_by_id`, run the
   classifier, and return `WorkerUnavailable` **before**
   `ExecutionWorkerJob::create_pending` and before any network call.
4. **API mapping** (`crates/server/src/error.rs`): add
   `ApiError::WorkerUnavailable(String)`, map it from the container variant
   above the `Container` catch-all, and render it as `503`,
   `WorkerUnavailableError`, with its message. Add a unit test.
5. **Transport cause** (`crates/services/src/services/cluster/client.rs`): have
   `Transport` render the `source()` chain. Add a unit test using a real
   `reqwest` error against an unroutable local port.
6. Run `cargo test -p services -p server -p local-deployment` (the
   targeted tests), `pnpm run format`, and clippy on the touched crates. Run
   `generate-types:check` if any TS-exported type changed (none is planned).
7. Codex review, then fix findings and repeat until the review is clean.
8. Knowledge base: add a vibe-kanban section on liveness gates for dispatch
   and on error chains, and a homelab page on worker cgroup memory throttling.
   Update both INDEXes.
9. Open PRs (vibe-kanban for code and docs, homelab for the SpecKit artifacts
   and KB), wait for CI, and merge.
