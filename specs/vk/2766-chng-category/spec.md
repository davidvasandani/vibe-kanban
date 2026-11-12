# Feature Specification: CHNG category

**Feature dir**: `specs/vk/2766-chng-category/`
**Status**: Clarified

## Summary
The operator files change tickets (Jira issue type CHNG) and usually works
them in an AI session. Today Vibe Kanban has no view of those tickets, the
Jira query that finds them can't be set anywhere, and starting a session for
one always starts cold, even when Hashmal (the operator's ticket dashboard)
already knows which earlier session created or worked the ticket. This
feature adds a CHNG list reachable from the left rail, a Settings page that
owns its Jira search, and a start-session flow that checks Hashmal first and
offers to continue the earlier session.

## User Stories
- US1: As the operator, I want a CHNG entry in the left rail so I can see my
  change tickets without leaving Vibe Kanban.
- US2: As the operator, I want to edit the Jira search (and its connection)
  in Settings so the list shows the tickets I care about.
- US3: As the operator, when I start a session from a CHNG ticket I want
  Vibe Kanban to find the session that created it in Hashmal and offer to
  continue that one first, so I don't lose context.

## Functional Requirements
- FR-1: The desktop left rail shows a "CHNG" item in a "Filters" section; the
  mobile navigation drawer shows the same entry. Selecting it opens the CHNG
  page and marks the item active.
- FR-2: The CHNG page lists the tickets returned by the configured Jira
  search: key (opens the ticket in Jira), summary, status, last updated. It
  can be refreshed. It says when results were cut off.
- FR-3: When the search is not configured, the page says so and links to
  Settings → CHNG. Jira failures show a message naming Jira and the cause.
- FR-4: Settings → CHNG holds the Jira site URL, account email, API token,
  the JQL search, the Hashmal MCP URL and the Hashmal token. The JQL starts
  as `project = CSEQ AND issuetype = CHNG AND reporter = currentUser() ORDER BY created DESC`
  and can be reset to that default.
- FR-5: Tokens are write-only: once saved, Settings shows only that a token
  is stored (or its 1Password reference). A token may be a literal or an
  `op://` reference. A token is only ever sent to the URL it was saved with;
  changing a URL requires entering the token again.
- FR-6: "Start session" on a CHNG ticket first asks Hashmal for the sessions
  linked to that ticket. While it looks, the dialog says so.
- FR-7: If Hashmal finds sessions, the dialog lists them, best match first
  (Hashmal's manual link, else the most recently active), with title, last
  activity and whether it is running, each with "Continue". "Start a new
  session" is always offered, after the matches.
- FR-8: "Continue" opens the new-workspace screen with a prompt that names
  the ticket and includes the recent transcript of the chosen Hashmal
  session, clearly labeled as context from an earlier session.
- FR-9: "Start a new session" opens the new-workspace screen with a prompt
  naming the ticket (key, summary, link) only.
- FR-10: If Hashmal is not configured, unreachable, does not track the
  ticket, or has no sessions for it, the dialog says which and still offers
  a new session. Hashmal problems never block starting a session.

## Out of Scope
- Editing CHNG tickets or writing anything to Jira or Hashmal.
- Resuming the original Hashmal process itself (it runs on another host).
- Creating Vibe Kanban issues for CHNG tickets.
- Any CHNG view in the remote (cloud) web app.
- Merging the Hashmal change (opened as its own PR for review).

## Acceptance Criteria
- [ ] The rail shows CHNG; clicking it opens `/chng` and highlights the item.
- [ ] With valid Settings, `/chng` lists the JQL's tickets; editing the JQL
      and saving changes the list on refresh.
- [ ] `GET /api/chng/settings` never contains a literal token.
- [ ] Changing the Jira URL without re-entering the token is rejected.
- [ ] Start session on a ticket Hashmal has sessions for shows them, primary
      first; Continue pre-fills the create screen with the ticket and the
      transcript excerpt.
- [ ] With Hashmal unconfigured or failing, the dialog explains and Start a
      new session works.
- [ ] Rust unit tests and Vitest cover settings rules, Jira/Hashmal parsing
      and prompt building.

## Open Questions
- None remaining (see Clarifications).

## Clarifications
- Session 2026-11-10: Hashmal access is a new read-only MCP tool in Hashmal,
  opened as a separate PR → VK degrades to "unavailable" until it ships.
- "Continue" = new VK session seeded with the earlier transcript; no
  "Open in Hashmal" link (no stable deep link).
- CHNG lives in the left rail, opening a page.
- Settings are host-scoped (this VK server), with their own Jira credential.
- No VK issue is created for a CHNG ticket.
- Which sessions count as "the session that created it": Hashmal's existing
  CHNG linking (manual link, else transcripts that mention the key, newest
  first). VK does not re-derive it.
