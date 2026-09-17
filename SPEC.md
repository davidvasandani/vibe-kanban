# Issue and workspace lifecycle

Task: vk/e464-issue-and-worksp

## Problem
A user resuming work by commenting on an archived workspace expects it to become active. When an archived workspace linked to a Done issue becomes active, the issue must return to In Progress.

## Requirements
- Accepting a new user comment/follow-up on an archived workspace automatically unarchives that workspace.
- Every archived-to-active workspace transition, including explicit unarchive and comment-driven activation, reopens its linked Done issue to the project's In Progress status.
- Preserve statuses other than Done; preserve already-active workspaces and unlinked workspace behavior.
- Apply behavior in backend lifecycle paths so UI and API clients behave consistently. Reuse existing issue/status synchronization conventions.
- Do not reactivate on reads or invalid requests. Cover transition boundaries and comment flows with focused regression tests.

## Scope and delivery
Only Vibe Kanban source is in scope. No changes to other services. Follow the requested knowledge recall and SpecKit stages before implementation; verify, independently review, document reusable knowledge, and open and merge a PR against the base branch.
