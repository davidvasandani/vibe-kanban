# VK MCP Auto Debug

Task: `9453-vk-mcp-auto-debu`

## Problem

When a saved MCP server connectivity test fails, the settings UI truncates the
diagnostic to one line. Users cannot reliably inspect or copy the complete
failure, and turning that diagnostic into actionable Vibe Kanban work requires
manually navigating to a project and re-entering the context.

## Goals

- Render the entire diagnostic returned for a failed MCP assignment test,
  preserving line breaks and long unbroken content without clipping it.
- Place a Copy action beside each failed result that copies the exact diagnostic
  string in full and gives accessible success/failure feedback.
- Place a Debug action beside each failed result when Settings was opened from
  an active local project route, creating a new issue in that project.
- Seed the issue with a concise title, the MCP server and executor identity, the
  exact full diagnostic, and explicit instructions for an agent to investigate,
  fix, test, and report the root cause.
- Prevent duplicate submissions while issue creation is in flight, report
  creation failures without discarding the MCP diagnostic, and keep Settings
  open after creation with a visible success link/action to the new issue.

## Functional Requirements

1. The enhanced actions appear for `failed` MCP assignment results. Existing
   connected, unsupported, and authentication-required behavior remains intact.
2. The displayed and copied diagnostic is `result.error` without frontend
   truncation or transformation. A missing diagnostic receives a localized
   fallback for display/copy/issue seeding.
3. Copy uses the browser clipboard and exposes an accessible confirmation; a
   clipboard error is visible and does not alter the diagnostic.
4. Debug creates the issue only in the active local project route context. If
   Settings was opened outside a project route, the UI explains that creation is
   unavailable rather than choosing an arbitrary project.
5. The seeded description uses Markdown fencing or equivalent safe formatting
   so multiline diagnostics remain legible, including diagnostics containing
   fence markers.
6. Existing MCP testing, OAuth connection, save, secret-redaction, and
   assignment semantics are not changed.
7. All new user-facing strings are localized through the settings translation
   namespace, with English as the source text and existing locale behavior
   preserved.
8. After Debug creates an issue, Settings remains open and shows a visible
   success link or action to the new issue. The UI does not navigate
   automatically.

## Technical Scope

The expected implementation is primarily in the shared web frontend MCP
settings result component, using existing project/issue mutation and navigation
infrastructure. Backend or shared API changes are permitted only if the existing
issue insertion contract cannot safely support this flow. Generated shared type
files must not be edited manually.

## Verification

- Component/unit tests cover full multiline rendering, exact clipboard payload,
  successful issue payload, unavailable project context, clipboard failure, and
  duplicate-click prevention.
- Existing MCP result/auth tests continue to pass.
- Run targeted frontend tests plus repository formatting and relevant type/lint
  checks.
- Independently review the final diff and resolve all significant findings.

## Non-Goals

- Automatically launching a workspace or coding agent for the new issue.
- Changing how MCP probes collect or sanitize backend diagnostics.
- Creating issues in third-party trackers or remote projects.
- Adding automated error classification or AI-generated remediation advice.
