# Implementation Plan: Legacy MCP Identifier Migration

1. Extend the constitution and VAS MCP specification with collision-safe legacy
   identifier migration requirements.
2. Model migration candidates from native snapshots without mutating files on
   read.
3. Return safe identifiers and preserved display labels to the shared settings
   editor only when the migration is unambiguous across profiles.
4. Make save atomically rename legacy native keys across assigned profiles and
   update display-label metadata; reject collisions before any write.
5. Add focused tests for `Atlassian Rovo`, credentials/assignment preservation,
   collisions, conflicts, and Codex-native output.
6. Update reusable knowledge, run formatting and focused Rust tests, then iterate
   independent Codex review until clean.
7. Commit, publish, merge, deploy, and verify the coordinator no longer stores
   the legacy Atlassian key.
