# Deploy Status in the Mobile Header

## Summary

Expose the running Vibe Kanban deployment's short Git SHA and elapsed time since deployment in the mobile navigation header. The desktop rail already exposes the SHA; this change makes equivalent deployment identity visible on narrow/mobile layouts without opening settings or switching views.

## Problem

The mobile UI header contains navigation and utility actions but no deployment identity. Operators therefore cannot tell which revision is running or how recently it was deployed from the mobile interface, even though the backend already reports the build version.

## Goals

- Display the running short Git SHA in the mobile header.
- Display a compact, human-readable elapsed duration since that deployment.
- Preserve the existing mobile navigation actions and usable layout at phone widths.
- Link a real SHA to its GitHub commit, consistent with the desktop UI.
- Keep local unstamped builds (`dev`) understandable and non-linking.
- Add focused automated coverage for presentation and responsive behavior where practical.

## Non-goals

- Changing deployment infrastructure or any service other than Vibe Kanban.
- Changing the desktop deployment indicator beyond extracting reusable presentation logic if useful.
- Adding a deployment history or release notes UI.
- Inferring deploy time from client page-load time.

## Functional requirements

1. The server info/config response must provide both the embedded Git revision and an authoritative build/deployment timestamp suitable for elapsed-time display.
2. The shared application layout must pass deployment metadata to the mobile navbar.
3. On mobile, the header must render a compact deployment indicator containing the short SHA and elapsed time (for example, `abc1234 · 2h`).
4. Elapsed time must update while the page remains open at an interval appropriate to its displayed precision.
5. A non-`dev` SHA must link to the matching `davidvasandani/vibe-kanban` commit in a new tab with safe external-link attributes.
6. The `dev` sentinel must render as plain text and must not produce a misleading GitHub link.
7. Missing deployment metadata must fail gracefully without an empty or broken control.
8. Existing refresh/update-available behavior must remain intact.

## UX and accessibility

- The indicator must remain legible but visually secondary to navigation.
- Its full accessible label/title must identify the deployed revision and elapsed time without relying only on compact abbreviations.
- It must not force the right-side mobile controls outside the viewport; truncation or responsive hiding of lower-priority text is acceptable at very narrow widths.
- Touch targets and existing header actions must retain their current behavior.

## Acceptance criteria

- A deployed production build shows its short SHA and time since deployment in the mobile header shown in the task screenshot.
- Tapping the SHA opens the exact GitHub commit.
- The elapsed label is derived from server-supplied deployment/build time and advances over time.
- A local `dev` build renders safely without a commit link.
- The mobile header remains functional at representative phone widths.
- Relevant frontend/backend tests and type checks pass.

## Constraints and risks

- Build metadata must be deterministic and embedded by the existing Rust build path; runtime filesystem or Git access is not assumed.
- Timestamp semantics must be documented clearly (build time versus service start time). If only build time is available, UI wording must avoid claiming stronger precision than the data supports.
- Generated TypeScript API types must only be changed through their Rust source generator.
