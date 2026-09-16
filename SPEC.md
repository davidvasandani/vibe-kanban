# Sidebar metadata reliability

Task: vk/113f-sidebar-randomly

## Problem
Workspace sidebar rows intermittently retain their names and pins while losing metadata and moving into Idle. The supplied screenshot shows the failure across many rows simultaneously.

## Required behavior
- Preserve accurate workspace metadata and activity grouping through routine refreshes and transient connection failures.
- Recover automatically when the authoritative data source reconnects.
- Apply real metadata changes, including explicit clearing, without retaining stale information indefinitely.
- Keep workspace identity and organization isolation intact.

## Technical scope
Trace sidebar rendering, workspace enrichment, streaming updates, and query lifecycles to establish the cause. Correct the smallest responsible boundary; avoid masking authoritative removals with unconditional sticky UI state. Limit changes to Vibe Kanban; no other services require changes.

## Acceptance
Regression coverage reproduces metadata loss and verifies recovery, valid updates, and workspace isolation. Run appropriate tests, type checks, lint and formatting, independent Codex review, record reusable knowledge, and open and merge a PR.
