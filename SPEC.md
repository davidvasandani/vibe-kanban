# Technical specification: intermittent workspace creation failure

Task: vk/40fb-workspace-creati (workspace 40fb8bd8-2342-4edf-a99c-11a11aaca27a)
Date: 2026-09-12

## Problem and scope
Users intermittently see “Workspace creation failed. Create a new workspace to try again.” Investigate the creation lifecycle and available operational evidence, identify the supported root cause, and fix it in Vibe Kanban. Hosting changes, if necessary, are limited to homelab/modules/vibe-kanban-rebuild.nix; other services are out of scope.

## Requirements
- Trace the displayed error to the backend creation/setup state and underlying failure.
- Preserve successful creation behavior, repository selection, branch isolation, and existing workspaces.
- Correct the evidenced intermittent failure without masking permanent errors or adding unsafe retries of side effects.
- Ensure failures remain diagnosable with useful context and no secret exposure.
- Add focused regression coverage for the identified failure and success path.

## Investigation and acceptance
Read existing project knowledge before planning. Follow the requested SpecKit workflow before implementation. Record evidence distinguishing confirmed causes from hypotheses. Install frozen dependencies, run appropriate tests and required formatting, obtain independent Codex review and address significant findings. Record reusable knowledge, commit changes, open and merge a PR against the repository base branch. Report any unavailable operational evidence or verification honestly.
