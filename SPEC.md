# MCP transport failure must not block unrelated Vibe Kanban work

Task: vk/669e-mcp-blocks-progr

## Problem
The user reports that a session working on Homelab/VAS-623/FAIKE cannot proceed after an rmcp HTTP transport worker reports a fatal channel closure for https://windows-mcp.vasandani.dev/mcp at 2026-09-18T14:10:33Z. The log identifies a failed MCP connection, but does not by itself establish that the agent process failed or why progress stopped.

## Scope
Investigate Vibe Kanban session/executor handling of MCP connection failures and implement the smallest evidence-backed correction. Changes are limited to Vibe Kanban and its deployment configuration. Repairing Windows MCP, FAIKE, Vercel, Squarespace, or another service requires scope clarification.

## Requirements
- Identify the affected session and correlate transport errors with agent turn status and terminal events.
- Distinguish a single optional MCP transport failure from an agent turn failure; preserve useful diagnostics without falsely ending or blocking unrelated work.
- Preserve actual executor failures and authentication/security boundaries.
- Keep working tools available and provide actionable recovery when the failed tool is needed.
- Add focused regression coverage for any confirmed failure-handling defect.

## Acceptance
Reproduce or establish the causal failure path from source and session evidence, implement and test the correction, complete SpecKit artifacts and independent review, record reusable knowledge, and open and merge a PR. Do not claim that the external Windows service is repaired based on changes to Vibe Kanban.

## Confirmed cause and selected correction
The coordinator stopped tracking this execution after a worker output replay gap, not an MCP-specific termination signal. A partially received thread/fork response and Codex's hydration warning identify the unnecessary historical-turn response burst. Request excludeTurns=true for fork and resume, retaining conversation context and explicit fallback history. Arbitrary-output journal redesign and recovery of already-lost output are outside this correction.
