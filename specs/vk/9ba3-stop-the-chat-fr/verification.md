# Verification: Stable Streaming Chat Viewport

## Implementation evidence

- Plan-exit detection is now edge-triggered by stable `patchKey` within a
  conversation scope.
- The first observation emits `plan`; repeated streaming snapshots ending in
  the same entry retain `running` and therefore follow the existing bottom/reader
  scroll policy instead of reissuing top alignment.
- A later distinct plan still emits `plan`, and conversation-scope reset restores
  first-observation behavior.
- Animation-frame batching retains a pending one-shot plan signal while adopting
  the newest snapshot, so high-frequency updates cannot erase the reveal before
  it renders.
- No virtualizer, tail-boundary, backend, or deployment behavior changed.

## Checks

- `pnpm install --frozen-lockfile` — passed.
- `pnpm --filter @vibe/web-core test -- plan-reveal-transition.test.ts` — passed;
  the package runner executed all 53 test files / 385 tests, all passing.
- `pnpm --filter @vibe/web-core check` — passed.
- `pnpm run local-web:check` — passed.
- `pnpm run remote-web:check` — passed.
- `pnpm run ui:check` — passed.
- `pnpm run local-web:lint` — passed. `web-core` has no ESLint configuration or
  lint script; its TypeScript and Prettier checks cover the changed shared code.
- `pnpm run format` — passed.
- `git diff --check` — passed.

## Visual reasoning

The pre-fix recording alternates between plan-top alignment and live-tail
alignment. After the first plan snapshot, the fixed transition can no longer
produce another `plan-reveal` for that entry, removing one side of that feedback
loop while leaving the active live-tail policy intact.
