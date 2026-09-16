# Verification: Timestamp the Workspace Chat Log

## Automated evidence

- `pnpm --filter @vibe/web-core test`: passed, 59 files / 418 tests.
- `pnpm run check`: passed across local web, remote web, web-core, UI, the main
  Rust workspace, and the remote Rust workspace.
- `pnpm run format`: passed; repository formatters completed cleanly.
- `git diff --check`: passed.
- `pnpm run lint`: frontend ESLint and both Rust Clippy runs passed. The final
  repository-wide unused-i18n-key check reported six pre-existing
  `metricsDiskAlerts.*` keys unrelated to this diff; this task changes no locale
  files or metric-alert code.
- Independent `codex review --uncommitted` found one P2 orphan-timestamp case
  for deliberately hidden tool entries. The renderer visibility rule and tests
  were added; the repeated review reported no actionable defects.

## Acceptance evidence

- Atomic normalized rows prefer their valid event timestamp and fall back to
  the authoritative execution-process creation time when the executor emits no
  event timestamp.
- Derived user prompts and script actions explicitly preserve process creation
  time.
- Aggregated tool/diff/thinking rows select their latest valid member time.
- Loading, next-action, and token-accounting controls remain untimestamped.
- Semantic `<time>` output preserves the source value and exposes a full local
  date/time through title and accessible-label metadata.
- Invalid values safely omit presentation or use a valid authoritative parent
  time; the browser clock is never used as an event-time fallback.
- Timestamp rendering is added inside the existing row wrapper; semantic keys,
  ordering, grouping, and virtualizer inputs are unchanged.

## Visual verification

The rendered-DOM test confirms the semantic and styling hook at the shared
row-metadata component. No seeded runnable workspace conversation was available
without creating external task/execution state, so no live browser mutation was
performed.
