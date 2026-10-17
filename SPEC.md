# Debug Codex refresh-token errors

## Problem

When Codex loses its session because a refresh token has been invalidated, its
internal logger writes the same authentication failure to stderr repeatedly.
Vibe Kanban currently renders those raw logger lines as ordinary error output.
The screenshot for this task shows the resulting wall of red text and provides
no concise, actionable recovery path.

The structured Codex protocol already classifies equivalent refresh failures
as setup-required errors. The stderr path does not apply that classification or
collapse duplicate logger output, so behavior depends on which Codex channel
reported the failure.

## Scope

Limit implementation to the Vibe Kanban source repository. The shared homelab
repository's `modules/vibe-kanban-rebuild.nix` is in deployment scope only if
diagnosis proves that its Codex refresh-broker wiring is incorrect. Do not
change any other service.

## Required behavior

- Detect Codex stderr records whose message reports a failed token refresh,
  including `refresh_token_invalidated` / revoked-session wording.
- Render one concise setup-required entry with practical recovery guidance
  instead of repeated raw internal log records.
- Preserve useful diagnostic context without exposing token material.
- Keep unrelated stderr output and structured Codex events unchanged.
- Avoid hiding a later distinct refresh failure merely because an earlier one
  was deduplicated.
- Work when a logger record is split across stream chunks or contains a
  multiline JSON error body.

## Acceptance criteria

1. The exact failure shape shown in the task attachment is covered by an
   automated regression test.
2. Repeated lines from one refresh failure produce a single user-visible,
   setup-required error.
3. The guidance tells a direct-login user to sign in again and retains the
   existing refresh-endpoint guidance for brokered cluster logins.
4. A non-authentication stderr line remains an ordinary error entry.
5. Relevant Rust tests, formatting, and repository checks pass.

## Non-goals

- Automatically renewing or replacing revoked credentials.
- Changing the Codex CLI's authentication protocol.
- Modifying unrelated services or broadening the homelab deployment topology.
