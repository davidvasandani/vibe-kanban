# Clarifications: Clustered Vibe Kanban

`/speckit.clarify` resolved all blocking questions from the task statement,
project constitution, existing homelab deployment conventions, and the
recommended architecture.

## Decisions

1. The target contract includes all delivery slices. The two-node Slice 2 pilot
   is a rollout gate, not a scope reduction.
2. The Nix module consumes externally managed credential files through systemd
   credentials; secrets never enter the Nix store or shared workspace volume.
3. `/srv/vibe-kanban-shared` is the configurable-but-identical mount root for
   `172.16.0.99:/var/nfs/shared/VibeKanban`.
4. Direct LAN HTTP/WebSocket is implemented first. Relay transport reuse remains
   an extension point and is not required to validate this task.
5. Existing local execution remains the default when cluster configuration is
   absent, allowing the feature to land and roll out by slice.

## Remaining Questions

None.
