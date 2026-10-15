# Feature Specification: Say which 1Password lookup failed, and why

**Feature dir**: `specs/vk/76ab-debug-1password/`
**Status**: Draft

## Summary

When Vibe Kanban starts an agent, it resolves organization Env Vars whose
values are 1Password references. When a lookup fails today, every failure
produces the same message: "1Password lookup failed; check the reference,
service-account token, and vault permissions". On 2026-10-15 this blocked
three follow-ups in SWE-278. Afterwards the reference, the token and the vault
access all checked out, and the most likely cause was the service account's
shared 24-hour request quota. That could not be confirmed, because the
provider's diagnostics were discarded. This feature makes the failure say
**which** variable failed and **what kind** of failure it was, while still
never revealing secrets or raw provider output (constitution LV, XLV).

## User Stories

- As an operator whose agent failed to start, I want the error to name the
  Env Var that failed, so that I know which org setting to inspect.
- As an operator, I want to tell "1Password is rate-limiting us" apart from
  "the token was rejected" and "the item isn't there or isn't shared", so that
  I either wait, rotate the token, or fix the reference, rather than checking
  all three.
- As a security-conscious admin, I want none of this to leak the reference
  target's value, the token, or 1Password's raw output.

## Functional Requirements

- FR-1: A failed lookup is reported as exactly one cause: *rate-limited*,
  *token rejected*, *not found or not shared*, or *unknown*. Each cause has
  its own remedy text.
- FR-2: When the failure comes from an organization Env Var, the message names
  that variable (for example `GH_TOKEN`).
- FR-3: When the failure comes from a GitHub owner token, the message keeps
  naming the owner and shows no internal placeholder name.
- FR-4: The cause is derived from the provider's diagnostics. Those
  diagnostics are bounded in size and never returned, logged or stored.
- FR-5: Operator logs record the variable name and cause of each failure,
  never the reference or the value.
- FR-6: Existing guarantees still hold: all-or-nothing resolution, the token
  is never on the command line, ambient provider settings are cleared, there
  is a total time limit, and the child process dies with the request.
- FR-7: An unrecognised failure keeps today's advice (check the reference,
  the token and vault permissions).

## Out of Scope

- Caching resolved org Env Vars to reduce quota use.
- Splitting or re-provisioning the homelab service account (homelab repo).
- UI changes. The message reaches the conversation as it does today.

## Acceptance Criteria

- [ ] A provider failure that reports rate limiting yields a message
      containing the variable name and saying the request was rate-limited.
- [ ] Representative provider messages map to the right cause: missing item,
      missing vault, missing field, malformed or rejected token, HTTP 429.
- [ ] A provider that prints secret-looking text on stdout and stderr and then
      fails yields an error whose display and debug forms contain neither
      text.
- [ ] GitHub owner-token failures read `GitHub token for <owner>: 1Password
      lookup failed: …`, with no internal key name.
- [ ] The existing environment-secret, GitHub-credential and owner-token tests
      still pass.

## Clarifications (resolved 2026-10-15)

- Q: Should a quota-exhausted failure retry automatically? **A: No. It tells
  the operator to retry once the quota resets.** Each retry spends more of
  the same exhausted quota, and the window resets after hours, not seconds,
  so an automatic retry would only delay the error. The remedy text points to
  `op service-account ratelimit` for the reset time.
- Q: Should timeouts name the in-flight variable? **A: No, not in this
  feature.** The 30-second budget covers locating the CLI and every lookup
  together, and today's timeout message already names its cause
  (connectivity). This stays out of scope, so the change remains small.

## Open Questions

None.
