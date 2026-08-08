# Technical Spec: Server Affinity Sidebar Polish (`61a3`)

## Objective

Polish the existing Server Affinity section in the workspace right sidebar so
its expanded content follows the sidebar's compact spacing rhythm and its
collapsed header continues to identify the workspace's current server.

## Scope

- Vibe Kanban frontend code only.
- Preserve the existing affinity query, migration/restart flow, placement
  choices, eligibility rules, and persistence behavior.
- Do not change any other service or deployment configuration.

## Current behavior

- The expanded Server Affinity body uses a full sidebar padding token and
  distributes each label/value row across the entire width. The resulting
  label-to-control gap is visually excessive, particularly for the “Run on”
  selector.
- The section header has access to affinity summary data, but the server label
  must be reliably visible while the section is collapsed, including when the
  server is represented by a worker hostname, requested hostname, or placement
  fallback.

## Required behavior

1. The expanded section uses compact, consistent horizontal and vertical
   spacing aligned with neighboring right-sidebar controls.
2. “Current server” and “Run on” remain readable at narrow sidebar widths; the
   selector occupies the available control column without causing overflow.
3. When the section is collapsed, the header shows a concise server name/status
   label on the right of “Server Affinity.”
4. Header text truncates safely rather than colliding with the disclosure icon.
5. The collapsed label resolves in this order: assigned worker hostname,
   requested worker hostname, then the translated placement-kind fallback.
6. Loading or temporarily absent summary data must not fabricate a server name
   or break section interaction.
7. Existing localization and accessibility behavior is retained.

## Implementation direction

- Reuse the existing section header extension point and affinity summary data;
  do not introduce a second network request for the collapsed label.
- Adjust layout classes in the affinity body using the established design
  tokens (`p-base`, `gap-half`, `gap-base`, text and width utilities).
- Keep data/state management in the container and avoid backend/schema changes.
- Add or update focused frontend tests where the repository's current component
  test seams allow the collapsed-header label and compact layout contract to be
  asserted without brittle pixel snapshots.

## Acceptance criteria

- With Server Affinity expanded, labels and values form a compact two-column
  layout with no oversized blank gap or horizontal overflow at the supported
  sidebar width.
- With Server Affinity collapsed, the current server (for example `think4`) is
  visible in the section header.
- Long server names truncate cleanly.
- Automatic, coordinator/local, assigned-worker, requested-worker, loading, and
  unavailable-summary states preserve meaningful existing fallbacks.
- Frontend formatting, type checks, linting, and focused tests pass.

## Non-goals

- Scheduler or worker eligibility changes.
- Affinity migration/restart behavior changes.
- Server Metrics redesign.
- Homelab module or deployment changes.
