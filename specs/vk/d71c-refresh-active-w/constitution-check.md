# `/speckit.constitution`: Active MCP Inventory Refresh

The existing Vibe Kanban constitution was refreshed against task
`vk/d71c-refresh-active-w`. No amendment is required.

The governing principles already cover this work directly:

- **II. Test the contract** requires acceptance criteria and automated evidence
  at the behavior boundary.
- **VI. Don't rebuild what shipped** requires investigation and extension of the
  existing active-refresh and restart-fallback machinery.
- **IX. External agent protocols are defensive contracts** prohibits assuming
  unsupported Codex protocol behavior.
- **XII. Asynchronous handoffs have one authoritative owner** governs queued
  reload/restart work and next-turn adoption.
- **XVII. Live capability state is confirmed and atomic** explicitly requires
  process-owned confirmation, complete generation replacement, preservation of
  last-known-good state, and truthful restart labeling.
- **XVIII. Distributed execution is affinity-bound and evidence-backed** governs
  coordinator-to-worker routing for clustered sessions.

Constitution gate: **PASS**. The feature specification and plan must test the
next-turn agent-visible registry rather than treating connector probing or a
reload acknowledgement as proof of adoption.
