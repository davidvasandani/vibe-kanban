# Fix Right Drawer Section Spacing

## Problem

The expanded **Server Affinity** section in the workspace right drawer is
treated as a flexible, fill-available-space panel. On a tall mobile viewport,
its two compact rows are consequently spread across a large empty section,
leaving "Current server" near the top and "Run on" near the bottom. The
controls should remain grouped at their natural content height.

## Scope

- Change only the Vibe Kanban frontend.
- Keep compact informational sections, specifically Server Affinity, at their
  intrinsic height when expanded.
- Preserve fill-available-space behavior for panels whose contents are intended
  to grow and scroll, such as Git, Changes, Logs, Preview, Browser, Server
  Metrics, Terminal, and Notes.
- Preserve the existing collapsed-section sizing, persistence, labels,
  controls, and responsive drawer behavior.
- Do not change any other service or homelab deployment configuration.

## Functional Requirements

1. Expanding Server Affinity must render its body immediately below its header
   without consuming the drawer's remaining vertical space.
2. "Current server" and "Run on" must retain the existing compact grid spacing
   and remain adjacent rows on desktop and mobile widths.
3. Collapsing Server Affinity must continue to hide its controls while keeping
   its header context visible.
4. Content-oriented sections must retain their ability to share available
   drawer height and scroll internally.

## Verification

- Add or update a focused component test proving Server Affinity is intrinsic
  while fillable sections remain flexible.
- Run the focused frontend tests and relevant type/lint checks.
- Format the repository before completion.
- Independently review the final diff and resolve significant findings.

## Acceptance Criteria

- The mobile right drawer no longer contains a large blank gap between the two
  Server Affinity rows.
- No unrelated service or deployment files change.
- Automated verification passes.
