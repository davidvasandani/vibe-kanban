# Data Model: Plan-Reveal Lifecycle State

## Existing inputs

- `AddEntryType`: `initial | running | historic | plan`.
- Latest timeline entry: a `PatchTypeWithKey` whose `patchKey` is stable for the
  logical normalized entry.
- Plan-exit predicate: normalized `tool_use` with tool name `ExitPlanMode`.

## New ephemeral state

`lastRevealedPlanPatchKey: string | null`

- Scope: one `useConversationHistory` conversation generation.
- Initial/reset value: `null`.
- Transition on a non-plan latest entry: unchanged.
- Transition on a plan latest entry with a different patch key: remember the key
  and emit `plan`.
- Transition on the same remembered key: retain the incoming update type.

No persisted or server-side entity changes.
