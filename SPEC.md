# Technical Specification: Timestamp the Workspace Chat Log

## Objective

Make the chronology of a workspace conversation explicit by showing a readable
timestamp on every visible chat-log item: messages, events, and actions.

## Problem

The normalized conversation stream carries timestamps for persisted log
entries, but the chat UI does not render them. Several client-derived entries
also discard the execution process time by setting their timestamp to `null`.
As a result, users cannot tell when a message was sent or when an agent event or
tool action occurred.

## Scope

This change is limited to the Vibe Kanban source repository and its workspace
chat UI. It does not change another service or homelab deployment configuration.

## Functional Requirements

1. Every visible normalized chat entry that represents a message, event, or
   action displays a timestamp.
2. Persisted normalized entries use their own server-provided timestamp.
3. Client-derived user-message and script entries use the authoritative
   execution-process creation time rather than the browser clock.
4. Grouped entries display a timestamp that accurately represents the grouped
   activity, without expanding the group.
5. Synthetic transient UI entries that do not represent a logged event (for
   example loading or navigation affordances) need not display a timestamp.
6. Missing or invalid timestamps degrade safely and do not render misleading
   dates or break the conversation.
7. Timestamp formatting follows the user's locale and exposes the full local
   date and time while keeping the chat log visually compact.

## UX Requirements

- Timestamps are secondary metadata and must not overpower message or action
  content.
- The compact display is understandable within a single-day conversation and
  the full local date/time is available accessibly and on hover.
- Existing expansion, edit, retry, reset, approval, and virtualization behavior
  remains unchanged.

## Technical Direction

- Preserve or derive timestamps at the conversation-entry derivation boundary.
- Add one reusable timestamp formatter/presentation primitive and apply it at a
  shared chat-row boundary where possible, with explicit handling for grouped
  rows.
- Keep locale formatting deterministic under test by separating timestamp
  validation/formatting from presentation.
- Do not edit generated shared types directly.

## Verification

Automated tests must cover:

1. Valid timestamps render as compact localized times with full date/time
   metadata.
2. Invalid or absent timestamps render no timestamp and do not throw.
3. Derived user messages and script actions inherit the execution-process
   creation time.
4. Message, event/action, and grouped-row rendering receive timestamp metadata.
5. Existing conversation row identity and virtualization semantics are intact.

Run focused frontend tests, type checking, linting, and repository-required
formatting appropriate to the affected packages.

## Out of Scope

- Database or API schema changes.
- Changing the ordering or retention of conversation entries.
- Timestamping composer drafts, loading indicators, or non-log controls.
- Changes to the homelab repository or any other deployed service.

## Acceptance Criteria

- A workspace chat visibly identifies when each logged message, event, and
  action occurred.
- The time shown comes from server-backed entry/process data.
- Missing or malformed timestamps are handled without regressions.
- Automated coverage passes and independent Codex review reports no significant
  findings.
- Reusable knowledge is recorded and the task pull request is merged.
