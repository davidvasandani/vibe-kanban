# Feature Specification: VK MCP Auto Debug

**Feature dir**: `specs/vk/9453-vk-mcp-auto-debu/`
**Status**: Draft
**Task**: `vk/9453-vk-mcp-auto-debu`

## Summary

When a saved MCP server connectivity test fails, Vibe Kanban should preserve the
full diagnostic as user-facing debugging evidence and make it easy to act on.
This feature expands failed MCP assignment results so users can inspect and copy
the complete backend diagnostic, then create a local Vibe Kanban issue seeded
with enough context for an agent to investigate, fix, test, and report the root
cause.

## Why

The current settings UI truncates failed MCP diagnostics to one line. That loses
the most useful evidence in many failures, especially multiline stderr, process
startup errors, and transport messages. Users who want an agent to debug the
failure must also leave settings, find a project, create an issue manually, and
paste the relevant context themselves. Preserving the exact diagnostic and
turning it into an explicit debug issue keeps the failure inspectable and
actionable without changing MCP probing, OAuth, assignment, or secret handling.

## User Stories

- As a user testing MCP server assignments, I want failed diagnostics to render
  in full, so I can read multiline errors and long process output without losing
  context.
- As a user debugging MCP connectivity, I want to copy the exact diagnostic
  string returned by the backend, so I can paste it into external tools,
  messages, or bug reports without accidental truncation.
- As a user who wants Vibe Kanban to investigate a failed MCP server, I want a
  Debug action on the failed result, so I can create an issue directly from the
  diagnostic.
- As a user creating a debug issue, I want the issue to include the MCP server
  name, executor identity, exact diagnostic, and clear investigation
  instructions, so an agent has the required context.
- As a user in a settings context with no explicit local project, I want issue
  creation to be unavailable with a clear explanation, so the app does not pick
  an arbitrary project for me.
- As a keyboard or screen-reader user, I want copy/debug feedback and errors to
  be announced accessibly, so I know whether the action succeeded.

## Functional Requirements

- FR-1: Enhanced diagnostic rendering and actions MUST appear only for MCP
  assignment test results with `failed` status.
- FR-2: Existing `ok`, `unsupported`, and `auth_required` result behavior MUST
  remain intact, including the existing OAuth Connect flow for authentication
  challenges.
- FR-3: The displayed diagnostic for a failed result MUST be the exact
  `result.error` string, preserving line breaks and long unbroken content
  without frontend truncation or transformation.
- FR-4: When `result.error` is absent or empty, the UI MUST use a localized
  fallback diagnostic for display, copying, and issue seeding.
- FR-5: Each failed result MUST expose a Copy action that writes the full
  diagnostic string to the browser clipboard.
- FR-6: Copy success and copy failure MUST be visible and accessible. Clipboard
  failure MUST NOT alter or discard the diagnostic.
- FR-7: Each failed result MUST expose a Debug action only when Settings is
  opened from an active local project route context.
- FR-8: When Settings is opened outside an active local project route context,
  the failed result MUST explain that debug issue creation is unavailable and
  MUST NOT choose an arbitrary project.
- FR-9: Debug issue creation MUST seed a concise title that identifies the MCP
  server and executor involved in the failure.
- FR-10: Debug issue creation MUST seed a Markdown description containing the MCP
  server identity, executor identity, the exact full diagnostic, and instructions
  for an agent to investigate the root cause, implement a fix, run relevant
  tests, and report the outcome.
- FR-11: The seeded description MUST format multiline diagnostics safely so they
  remain legible even when the diagnostic itself contains Markdown fence
  markers.
- FR-12: Debug issue creation MUST prevent duplicate submissions while creation
  is in flight.
- FR-13: Debug issue creation failures MUST be reported without discarding,
  truncating, or transforming the MCP diagnostic.
- FR-14: After a debug issue is created, Settings MUST remain open and the
  failed result MUST show a visible success link or action to open the new
  issue. The UI MUST NOT navigate automatically.
- FR-15: The feature MUST NOT change MCP testing, saved MCP configuration,
  secret redaction, assignment compatibility, settings save/discard behavior, or
  OAuth connection semantics.
- FR-16: All new user-facing strings MUST be localized through the settings
  translation namespace, with English source text and existing locale behavior
  preserved.

## Out of Scope

- Automatically launching a workspace or coding agent for the new issue.
- Changing how MCP probes collect, sanitize, or classify backend diagnostics.
- Creating issues in third-party trackers, remote projects, or arbitrary local
  projects.
- Adding automated error classification, suggested fixes, or AI-generated
  remediation advice.
- Changing MCP server storage, assignment compatibility rules, OAuth credential
  refresh, or secret materialization.

## Acceptance Criteria

- [ ] A failed MCP assignment result displays the complete diagnostic with
      line breaks and long unbroken content visible, without one-line clipping.
- [ ] The Copy action writes exactly the displayed diagnostic string to the
      clipboard.
- [ ] Copy success and copy failure produce accessible feedback, and copy
      failure leaves the diagnostic unchanged.
- [ ] The Debug action creates an issue in the active local project route
      context when Settings was opened from a project route.
- [ ] The created issue title identifies the MCP server and executor associated
      with the failed assignment.
- [ ] The created issue description includes the exact full diagnostic and safe
      Markdown formatting for multiline text, including diagnostics containing
      fence markers.
- [ ] Repeated Debug clicks while issue creation is pending create at most one
      issue.
- [ ] If Settings was opened outside a project route, the UI disables or
      withholds Debug issue creation and explains why.
- [ ] After successful issue creation, Settings remains open and shows a
      visible success link or action to the new issue without automatic
      navigation.
- [ ] Issue creation failure is visible to the user and does not remove or
      mutate the failed diagnostic.
- [ ] `ok`, `unsupported`, and `auth_required` MCP assignment results retain
      their existing behavior and controls.
- [ ] Existing MCP testing, OAuth connection, settings save/discard, and
      assignment semantics continue to pass their current tests.
- [ ] Focused component or unit tests cover multiline rendering, exact clipboard
      payload, successful issue payload, unavailable project context, clipboard
      failure, and duplicate-click prevention.
- [ ] Repository formatting and relevant frontend validation pass.

## Assumptions

- The existing failed assignment result contains enough stable identity to label
  the MCP server and executor in a debug issue.
- The active project route context is the only explicit project context for
  debug issue creation.
- Existing local issue creation and navigation infrastructure can support this
  flow without backend schema changes.
- Non-English locale files may use the repository's current fallback convention
  if direct translation is not required by validation.

## Clarifications

- The explicit project is the active project route context only. If Settings is
  opened outside a project route, Debug is unavailable.
- After creating a debug issue, keep Settings open and show a visible success
  link or action to the new issue. Do not navigate automatically.
