# Feature Specification: Restore Opus 4.8 in the Claude Code model picker

**Feature dir**: `specs/vk/a221-restore-opus-4-8/`
**Status**: Draft

## Summary
The Claude Code model picker used to offer Opus 4.8. It disappeared when the
catalog was rebuilt around versioned model IDs, even though nobody asked for its
removal and the pinned Claude Code CLI still supports it. Users who want to run
a task on Opus 4.8 (for example, to compare behaviour with Opus 5.x, or because
Opus 4.8 is the model the CLI itself falls back to) cannot pick it. This
feature puts Opus 4.8 back in the list with the same effort controls and
context-usage reporting as the other current models.

## User Stories
- As a Vibe Kanban user, I want to choose "Opus 4.8" from the Claude Code model
  picker so that a session runs on `claude-opus-4-8`.
- As a user running Opus 4.8, I want the effort selector (Low through Max) so
  that I can tune it like the other Opus models.
- As a user running Opus 4.8, I want the context gauge to measure against the
  model's real 1M-token window so that it does not show the session as full too
  early.
- As a user who hides models I don't use, I want Opus 4.8 to respect my hidden
  models setting like every other entry.

## Functional Requirements
- FR-1: The Claude Code picker lists "Opus 4.8", which launches `claude-opus-4-8`.
- FR-2: Opus 4.8 appears in newest-first order, directly after Opus 5 and before
  the Sonnet models.
- FR-3: Opus 4.8 offers the effort levels low, medium, high, xhigh and max.
- FR-4: The context window used for Opus 4.8 sessions is 1,000,000 tokens.
- FR-5: The default model and the shorthand-alias resolution (`opus`,
  `sonnet`, `fable`, `haiku`) are unchanged.
- FR-6: Opus 4.8 can be hidden and unhidden through the existing per-agent
  hidden-models setting. It is visible by default.

## Out of Scope
- Changing which model the Claude Code CLI uses when it retries after a
  safety refusal.
- Granting account access to Opus 4.8. If the account can't use it, the
  provider's own error is shown, as for any other model.
- Changing the pinned Claude Code CLI version.
- Other agents (Cursor, Copilot, Droid already list Opus 4.8).

## Acceptance Criteria
- [ ] The Claude Code catalog's ordered model list includes
      `claude-opus-4-8` / "Opus 4.8" directly after Opus 5, and a regression test
      enforces the full ordered list.
- [ ] Opus 4.8 carries exactly the effort options low, medium, high, xhigh, max.
- [ ] The context window for `claude-opus-4-8` is 1,000,000 tokens (unit test).
- [ ] The default model remains `claude-opus-5-5`, and the alias map is unchanged
      (existing tests pass).
- [ ] The executor test suite, type generation check, formatting and lint pass.

## Open Questions
- None. The display label follows the existing "Opus 5.5" pattern and the pinned
  CLI's own `display_name` ("Opus 4.8"). The placement follows the newest-first
  rule. Capabilities come from the CLI's catalog record.

## Clarifications

### Session 2026-11-23
- Q: Which list does "this list" mean? → A: The Claude Code chat model picker
  shown in the user's screenshot (Haiku 5.5, Mythos 5.2, Sonnet 5.5, Fable 5.2,
  Opus 5.5). That menu reflects the executor catalog minus the user's hidden
  models.
- Q: Should Opus 4.8 become the default, or an alias target? → A: No. The user
  asked to restore it to the list. The default stays Opus 5.5, and the CLI's
  `opus` alias points at Opus 5.5.
- Q: Should the 1M window use a `[1m]` variant? → A: No. The pinned CLI marks
  Opus 4.8 `native_1m`, so the plain ID already has a 1M window. No separate
  "Opus 4.8 (1M)" entry.
- Q: What happens for accounts without Opus 4.8 access? → A: The provider error
  is shown unchanged, as for Mythos 5.2 (out of scope).
