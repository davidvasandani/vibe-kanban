# Feature Specification: Coordinator Workspace Placement

**Feature dir**: `specs/vk/2fe7-vk-coordinator-m/`
**Status**: Draft

## Summary

Allow users of a clustered Vibe Kanban deployment to deliberately create a workspace on the coordinator. The create form currently exposes automatic placement and registered workers only, leaving a supported execution location unavailable as an explicit choice.

## User Stories

- As a cluster user, I want to choose the coordinator in the **Run on** menu so that I can deliberately keep a workspace on the coordinator.
- As an operator, I want coordinator intent to remain distinct from automatic scheduling so that the system never sends deliberately local work to a worker.
- As an API client, I want existing workspace creation requests to preserve their behavior so that this UI capability does not break older clients.

## Functional Requirements

- FR-1: The workspace creation placement selector must offer **Coordinator** in addition to automatic placement and registered workers.
- FR-2: Selecting **Coordinator** must express explicit coordinator-local placement intent.
- FR-3: Coordinator-local placement must not invoke worker scheduling or reserve a worker.
- FR-4: A coordinator-local workspace must retain the system's established local placement state and use the existing local execution lifecycle.
- FR-5: Automatic placement must retain its existing scheduling behavior.
- FR-6: Selecting a registered worker must retain its existing manual worker-placement behavior.
- FR-7: Existing API clients that do not express coordinator-local intent must retain their current behavior.
- FR-8: The system must reject a request that simultaneously expresses coordinator-local intent and names a worker.
- FR-9: Contradictory placement intent must be rejected before worker reservation or any other placement mutation.
- FR-10: Non-clustered deployments must retain their existing local workspace creation behavior.

## Out of Scope

- Changing worker eligibility, scheduling weights, lease handling, or mount-health rules.
- Registering the coordinator as a synthetic worker.
- Changing cluster deployment topology or another homelab service.
- Moving an existing workspace between the coordinator and a worker.

## Acceptance Criteria

- [ ] The **Run on** menu renders **Automatic placement**, **Coordinator**, and the existing registered worker choices.
- [ ] Creating with **Coordinator** results in a local placement with no worker ID and starts through coordinator-local execution.
- [ ] Creating with **Automatic placement** continues to select an eligible worker.
- [ ] Creating with an explicit worker continues to reserve that worker.
- [ ] A request containing both coordinator intent and a worker ID returns a clear bad-request error before placement changes.
- [ ] A request from an older client that omits coordinator intent behaves exactly as before.
- [ ] Focused backend and rendered frontend tests cover the new choice and unchanged placement modes.

## Open Questions

None.
