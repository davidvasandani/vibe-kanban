# Constitution Check

`/speckit.constitution` was run for `vk/af0d-no-conversation`. The existing
Vibe Kanban constitution remains current; no amendment is required.

The task is governed especially by:

- II, test the contract with focused Rust coverage;
- III and VI, use the smallest change and the existing thread-start machinery;
- IX, treat the Codex protocol defensively, preserve session identity, and fail
  loudly for errors outside the verified recovery case;
- XII, keep the external-session handoff authoritative; and
- XVIII, preserve workspace affinity and worker-owned execution evidence.
