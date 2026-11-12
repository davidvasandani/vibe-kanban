# Local server API (all under `/api`, `ApiResponse` envelope)

## GET /chng/settings → `ChngSettings`
Never includes a literal token.

## PUT /chng/settings (`UpdateChngSettingsRequest`) → `ChngSettings`
400 when: URL invalid; email missing while a Jira URL is set; JQL > 2000
chars; a URL changed while its token stays stored and no new token is given
("re-enter the token"); token invalid (empty after trim when replacing is
treated as clear; > 4 KiB or NUL rejected).

## GET /chng/tickets → `ChngTicketList`
- Not configured (no URL/email/token) → `configured: false`, empty list.
- Jira 401/403 → 502 "Jira rejected the API token (stored literal | from 1Password)".
- Jira 400 → 400 "Jira rejected the search: <bounded errorMessages>".
- Network/timeout → 502 "Jira is unreachable".

## GET /chng/tickets/{key}/hashmal[?session_id=] → `HashmalChngLookup`
Always 200 for a valid key (400 for an invalid key). Hashmal problems are
`state: unavailable` + `message`.
