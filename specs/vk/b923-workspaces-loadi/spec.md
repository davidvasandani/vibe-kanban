# Feature Specification: Workspaces sidebar metadata loads and recovers on every device

**Feature dir**: `specs/vk/b923-workspaces-loadi/`
**Status**: Draft

## Summary
On mobile, the Workspaces sidebar showed workspace names and pins, but every
row was blank beneath its title. There was no elapsed time, no diff stats, no
host label and no PR badge, and "Needs Attention" and "Polling" were empty even
when work needed attention. That metadata arrives in one bulk request, and two
problems stop it from arriving:

- The request waits for the slowest workspace's change statistics. Measured
  response times range from 0.27 s to 16.6 s.
- The client gives that request no deadline. A request that never settles
  (common on phones that suspend or switch networks) captures every later
  refresh, so the sidebar stays blank until the page is reloaded.

This feature makes the metadata arrive quickly on every device, stay fresh,
and recover by itself without a reload.

## User Stories
- As a user on my phone, I want each workspace row to show its status,
  elapsed time, changes and host soon after I open the app, so that I can tell
  which workspaces need me without opening each one.
- As a user who backgrounds the app and comes back, I want the sidebar to
  refresh on its own, so that I never have to reload the page to get the
  metadata back.
- As a user with many workspaces, I want one slow workspace not to hold back
  everyone else's status, so that "Needs Attention" is accurate even when git
  is slow.
- As a user, I want a row that already showed change counts to keep showing
  them while a refresh is slow, so that the list does not flicker to blank.

## Functional Requirements
- FR-1: The bulk workspace summaries response is returned within a bounded
  time budget, however slow any single workspace's change statistics are.
- FR-2: Metadata that is cheap to produce (latest process status and time,
  pending approvals, unseen activity, poller and dev-server state, PR, host
  affinity) is never delayed by change statistics.
- FR-3: When a workspace's fresh change statistics are still being computed at the budget, the
  response carries that workspace's most recently observed statistics. If none
  have ever been observed, it carries none. It never carries invented values.
- FR-4: Statistics work that was already started for a request still finishes
  and is made available to the next request. Missing the budget causes no
  extra work, and existing sharing and concurrency limits are preserved.
- FR-5: Every client request for summaries has a deadline. A request that has
  not settled by then is abandoned and counts as a failed refresh.
- FR-6: A failed or abandoned refresh keeps the last successfully displayed
  metadata for the same host and archive scope. The next scheduled refresh
  sends a new request instead of waiting on the abandoned one.
- FR-7: When refreshes stop being needed (the view goes away or the host
  changes), the in-flight summaries request is cancelled.
- FR-8: When the app becomes visible or focused again, summaries refresh
  promptly.
- FR-9: The response format and the generated shared types stay unchanged.

## Out of Scope
- Pushing summaries over the live workspace stream.
- Changing freshness tiers for change statistics, the 14-day idle skip, or the
  git concurrency limit.
- Sidebar sections, sorting, filtering and layout.
- Proxy, tunnel or hosting configuration (homelab repo).

## Acceptance Criteria
- [ ] AC-1: When a workspace's statistics recompute outlasts the budget, the
  summaries response still arrives, and the workspace shows its previous
  statistics. A later request shows the recomputed values without computing
  them again.
- [ ] AC-2: A workspace whose statistics have never been computed, and whose
  first computation outlasts the budget, shows no statistics in that response.
  It shows the computed values on a later request.
- [ ] AC-3: After a workspace's statistics are invalidated (a process exited),
  a request still recomputes them when possible within the budget. The
  previous values are used only as a fallback.
- [ ] AC-4: A client summaries request that never settles is abandoned at the
  deadline. The sidebar keeps its earlier metadata, and the next scheduled
  refresh fetches and shows new metadata with no reload.
- [ ] AC-5: Cancelling the summaries query cancels the underlying network
  request.
- [ ] AC-6: Making the page visible or focused again triggers a new summaries
  request.
- [ ] AC-7: Existing behaviour holds: failed refreshes keep the last snapshot,
  successful empty snapshots clear metadata, and host switches never show
  another host's data.
- [ ] AC-8: After deployment, repeated calls to the live summaries endpoint
  complete within the budget plus normal database time.

## Clarifications (resolved)
- **Budgets.** Server-side budget: **3 s** for the whole statistics phase of
  one request, not per workspace. Client deadline: **20 s**. Evidence: warm
  requests take about 0.3 s, and cold ones took up to 16.6 s. 3 s keeps the
  sidebar responsive while still letting most recomputes finish in time. The
  20 s client deadline sits well above the server budget plus database time
  and a slow mobile link, so healthy requests are never cut off. It also
  bounds how long a request that never settles can block refreshes (at most
  about 20 s plus one 15 s poll).
- **Failed versus slow computation.** Only a computation that has **not
  finished** within the budget falls back to the last observed statistics. A
  computation that **finished and failed** reports no statistics, as it does
  today. A failure can mean the worktree is gone or broken, and showing old
  numbers for it would be misleading. Refines FR-3.
- **The empty "Running" section.** Out of scope. `Running` comes from the live
  workspace stream (`is_running`), which reconnects and keeps its snapshot. No
  evidence points to it being wrong at the time of the screenshot, and the live
  check showed Running populated correctly. If it recurs, it should be filed on
  its own.

## Open Questions
- None remaining.
