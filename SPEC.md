# Technical Spec: Coordinator Placement Option

## Problem

In clustered deployments, the workspace creation form offers automatic placement and individual worker nodes, but it does not offer the coordinator. Operators therefore cannot deliberately run a new workspace on the coordinator even though coordinator-local execution remains a supported placement state.

## Scope

This change is limited to the Vibe Kanban service. It updates the workspace-creation contract, coordinator placement handling, and the create-workspace UI. It does not alter worker registration, scheduling weights, cluster deployment topology, or any other homelab service.

## Required behavior

1. The **Run on** selector shows a **Coordinator** option alongside **Automatic placement** and eligible worker nodes.
2. Selecting **Coordinator** sends an explicit coordinator-placement intent. It must not be represented as automatic placement, because automatic placement remains free to select a worker.
3. When coordinator placement is requested in cluster mode, creation retains the workspace's initial `local` placement and starts it through the existing coordinator-local execution path. The worker scheduler is not invoked.
4. Existing clients that omit the new intent preserve current behavior: `requested_worker_node_id = null` means automatic worker scheduling, while a worker UUID means manual worker placement.
5. A request must not specify both coordinator placement and a worker UUID. The server rejects that ambiguous request with a clear bad-request response.
6. Non-clustered installations continue to use local execution without regression.
7. The selector continues to distinguish unavailable worker nodes and does not make their existing state rules less strict.

## Technical approach

- Add an additive, default-false `run_on_coordinator` field to the create-and-start workspace request.
- In clustered workspace creation, branch before worker selection: validate that coordinator intent and a worker UUID are mutually exclusive; retain the initial local placement for coordinator intent; otherwise execute the existing scheduler/reservation flow unchanged.
- Add a stable coordinator sentinel value only in UI state, translating it to `run_on_coordinator: true` and `requested_worker_node_id: null` at the API boundary.
- Add focused backend tests for coordinator placement and conflicting intent, plus frontend coverage for rendering and request serialization.
- Regenerate shared TypeScript types from the Rust source rather than editing generated output manually.

## Acceptance criteria

- The screenshot's **Run on** menu includes **Coordinator**.
- A workspace created with that option has `placement_state = local`, no worker node, and starts successfully on the coordinator.
- Automatic and explicit-worker selections behave exactly as before.
- Conflicting coordinator/worker intent returns HTTP 400 and does not partially reserve a placement.
- Relevant Rust and frontend tests, type generation checks, formatting, and lint/type checks pass.

## Risks and mitigations

- **Ambiguous null semantics:** keep automatic placement as the existing null worker value and carry coordinator intent in a separate explicit boolean.
- **Partial creation on invalid input:** validate mutual exclusivity before scheduler or placement mutation.
- **Generated-type drift:** regenerate with the repository command and verify the generated-types check.
