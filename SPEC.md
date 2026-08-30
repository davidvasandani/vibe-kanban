# VAS-448 Message Send Failure Investigation

## Problem

Sending a follow-up message from the Vibe Kanban workspace associated with task
`VAS-448` fails in the web UI with:

> Failed to send: An internal error occurred. Please try again.

The displayed text is Vibe Kanban's sanitized response for an unclassified
server-side error, so it does not identify the underlying failure.

## Scope

- Trace the workspace-chat send request from the web client through the Vibe
  Kanban API and executor/session layers.
- Correlate the failing request with available Vibe Kanban runtime logs and the
  persisted workspace/session state for `VAS-448`.
- Correct the Vibe Kanban source or its deployment configuration in
  `homelab/modules/vibe-kanban-rebuild.nix` if a reproducible defect is found.
- Preserve unrelated services and configuration.

## Requirements

1. Identify the concrete server-side error hidden by the generic API response.
2. Determine why the error is specific to, or exposed by, the workspace for
   `VAS-448`.
3. Add regression coverage at the narrowest practical layer for any code fix.
4. Keep externally returned errors sanitized while retaining actionable
   diagnostics in server logs.
5. Verify relevant formatting, tests, and static checks.

## Acceptance Criteria

- A message can be sent successfully to the affected active session, or the
  investigation produces a precise, evidence-backed operational cause and an
  in-scope remediation.
- The UI no longer receives an unclassified internal error for the identified
  condition.
- Regression tests cover the failure mode when the remediation changes code.
- An independent Codex diff review reports no significant findings.
- Reusable findings are recorded in the project knowledge base and indexed.

## Non-goals

- Changes to services other than Vibe Kanban.
- Broad redesign of workspace chat or executor protocols.
- Exposing internal exception details to browser clients.
