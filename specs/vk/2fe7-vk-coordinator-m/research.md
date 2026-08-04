# Research: Coordinator Workspace Placement

## Existing behavior

- New workspace records begin with a local placement.
- When cluster mode is enabled, `create_and_start_workspace` always calls `WorkerScheduler::select` and reserves the selected worker. A null `requested_worker_node_id` means automatic worker scheduling, not coordinator-local execution.
- The create form currently stores either the string `automatic` or a worker UUID and only renders the selector when at least one worker row exists.
- The coordinator is not represented by a `WorkerNode`, and treating it as one would incorrectly mix local execution with worker leases, mount health, and capabilities.

## Decision

Use a separate additive boolean for coordinator intent and resolve the boolean plus optional worker UUID into a closed internal intent enum. This preserves the existing worker field's meaning and avoids a synthetic worker or magic UUID on the wire.

## Alternatives rejected

- **Use null worker ID for coordinator:** rejected because null already means automatic scheduling in cluster mode.
- **Use the coordinator UUID as a worker UUID:** rejected because the scheduler searches registered worker rows and the coordinator is deliberately not one.
- **Add a synthetic coordinator to the worker list:** rejected because worker settings, eligibility, leases, and mount status do not apply to the coordinator.
- **Replace the contract with a new tagged enum:** clean in isolation but unnecessarily breaking for existing clients; the additive field provides an incremental migration.

## Dependencies

No new dependencies.
