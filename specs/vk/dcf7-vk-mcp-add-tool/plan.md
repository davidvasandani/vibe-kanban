# Technical Plan: MCP Tool Count and Last-Checked Time

**Spec**: `./spec.md`  
**Status**: Ready for tasks

## Technical context

The implementation is React 18 + TypeScript in `packages/web-core`, using
i18next, Tailwind design tokens, and Vitest. Shared test results already expose
`McpServerTestResult.tool_count: number | null`; no Rust or generated contract
work is necessary.

## Architecture and approach

Add a small pure helper in `packages/web-core/src/shared/lib/` that accepts the
assignment test results for a logical server and produces either no count, one
count, or a minimum/maximum range. Keep localization outside the helper so its
contract remains data-only and tests do not require i18n setup.

`McpSettingsSection` will own a second transient map:
`Record<string, number>`, keyed by logical server name and storing epoch
milliseconds. When `testSharedMcpAssignments` resolves, capture `Date.now()`
once, derive the unique `server_name` values returned, merge results as today,
and merge timestamps for precisely those servers. Existing result-reset sites
will also reset timestamps.

At card render time, select all indexed results for the server's assignments,
derive the aggregate tool summary, and combine it with the stored timestamp.
Render a muted wrapping metadata line below assignment badges and above actions.
Use i18next pluralization for equal counts, a separate range string for differing
counts, and `Intl.DateTimeFormat(i18n.resolvedLanguage ?? i18n.language, ...)`
for a locale-aware compact timestamp.

## Data model

See `./data-model.md`. All data is client-only and ephemeral.

## Contracts

See `./contracts.md`. There is no network contract change.

## Research notes

See `./research.md`. No new dependency is introduced.

## Constitution check

- **I Clarity**: a named pure aggregator avoids complex inline card logic.
- **II Test the contract**: aggregation and time formatting receive focused
  Vitest coverage; relevant TypeScript and formatting checks are required.
- **III Small/reversible**: reuse the existing backend `tool_count` and current
  card/result state rather than adding persistence or API fields.
- **IV Shared boundaries**: the feature remains in `web-core`, the existing
  shared settings implementation used by both frontends.
- **VI Don't rebuild**: builds directly on the shipped MCP testing path and
  knowledge-base lifecycle rules.
- **XI Diagnostics**: existing diagnostic content and actions are untouched.
- Generated types are not edited, no dependency is added, and full repository
  formatting runs before completion.

No constitution deviations or open questions remain.

## Risks and mitigations

- **Multi-assignment ambiguity**: aggregate all successful known counts and show
  a range if they differ.
- **Stale configuration metadata**: clear timestamps everywhere test results are
  cleared after a configuration reload/save.
- **Retest collateral updates**: timestamp only unique server names actually
  returned by the API.
- **Locale support variance**: rely on browser `Intl.DateTimeFormat` and i18next
  locale fallback rather than hand-built date strings.
- **Responsive crowding**: use a separate muted, wrapping line rather than adding
  more badges to the title row.

## Verification

1. Run the new helper's Vitest file.
2. Run `pnpm --filter @vibe/web-core check`.
3. Run relevant MCP library tests.
4. Run `pnpm run format`, then inspect the diff.
5. Run independent Codex review and repeat checks after fixes.
