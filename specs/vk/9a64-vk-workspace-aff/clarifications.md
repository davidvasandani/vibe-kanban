# Clarifications: Workspace Server Affinity and Migration

## 1. Persistent processes during migration

**Decision:** Block affinity changes while a dev server or background helper is running.

**Reasoning:** These processes have different persistence and restart semantics from a coding-agent turn. Their arbitrary commands cannot be recreated safely from a generic continuation prompt, and leaving them alive on the previous worker would make displayed ownership false. The operator receives an actionable instruction to stop them first.

## 2. Automatic placement on a stopped workspace

**Decision:** Re-run placement immediately and persist the selected worker while leaving `requested_worker_node_id` empty.

**Reasoning:** The requested UI is an affinity control whose result must be visible immediately. Merely clearing a preference while retaining a stale current worker would make the drawer ambiguous. The latest coding-agent profile supplies scheduler capability constraints.

## 3. Explicit coordinator selection

**Decision:** Do not offer the coordinator as an explicit target in clustered mode. In non-cluster mode, show local placement and make the section informational.

**Reasoning:** The coordinator has no `worker_nodes` identity or worker execution endpoint. Inventing a sentinel ID would cut across established authority and routing contracts. Automatic placement remains the product's unconstrained option.

## 4. Multiple running coding-agent executions

**Decision:** Migration requires exactly one running coding-agent execution. Its session and persisted executor profile are authoritative. Zero running executions uses the stopped path; more than one returns a conflict without mutation.

**Reasoning:** A continuation is session-scoped, and choosing “latest” while another process writes would risk concurrent edits and duplicate work. Multiple active coding agents violate the current workspace lifecycle expectation and should be exposed rather than guessed around.

## Remaining questions

None.
