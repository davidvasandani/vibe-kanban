# CHNG category: Jira search, Hashmal lookup, and continuing a session from another host

The left rail's **Filters → CHNG** item opens `/chng`. That page lists the
operator's change tickets from a Jira search set in **Settings → CHNG**.
**Start session** on a ticket first asks Hashmal, the operator's ticket
dashboard in `sweetgreen/platform-ops` `apps/hashmal`, which earlier Claude
session worked the ticket, and offers to continue it. This page records the
decisions that aren't obvious from the code.

## Hashmal is reached only through its MCP endpoint

- Hashmal's `/api/filters/chng` uses Django session auth, so a server can't
  call it. `/mcp` is the only surface that takes a token: a stateless
  JSON-RPC POST with a bearer `hsm_…` token, JSON replies only (no SSE). A bare
  `tools/call` works without `initialize`.
- VK calls the read-only tool `hashmal_chng_sessions(key, session_id?, limit)`
  (platform-ops PR #350). It resolves sessions exactly as Hashmal's
  Filters/CHNG view does: the manual link first, `cleared` hides every match,
  otherwise transcripts that mention the key, newest first. It never scans
  transcripts on a request, because cold scans hang Hashmal's workers. A
  `session_id` that isn't one of the ticket's sessions is refused.
- `services::chng::hashmal_lookup` never returns `Err`. Every failure becomes
  `state: unavailable` with a classified message, so Hashmal can never block a
  fresh session. The cases:
  - unknown tool (Hashmal not yet updated);
  - 401/403, which names the credential source;
  - a non-JSON body, usually a Cloudflare Access login page;
  - unreachable.

## "Continue" is a new session seeded with the transcript

Hashmal's sessions are Claude transcripts on another host (macmini-m2), so VK
can't `--resume` them. Constitution XLVI also forbids fabricating vendor
history. "Continue" therefore starts a new VK workspace through
`setCreateModeSeedState` + `goToWorkspacesCreate()`. Its first prompt
(`shared/lib/chngPrompt.ts`) contains:

- the ticket;
- a statement that the earlier session is not resumed;
- the newest transcript turns in an `<earlier-session-transcript
  source="hashmal" session=…>` block, framed as reference context and not as
  instructions (LVII), and capped at about 24k characters. A turn is either
  kept whole after its own clip or dropped.

## Settings → CHNG storage

- The settings live in a singleton table, `chng_settings` (`id = 1`). They are
  never stored in `Config`, for the same reason as
  [[github-owner-token-routing]].
- Tokens are sealed under their own host key, `chng_settings_aead_key`, with
  AAD `vk-chng-settings|<service>|<url>`. **The URL is part of the binding**,
  so an envelope only decrypts for the destination it was saved with.
- On top of that, changing a URL without re-entering its token is rejected.
  `save_if_unchanged` makes that check atomic with the write: the `UPDATE …
  WHERE` compares the URLs and the token envelopes. A cleared URL drops its
  token.
- Each token field in an update is absent to keep it, `""` to clear it, or a
  value to replace it. Only `op://` references are ever returned. Server-side
  resolution goes through `github_credentials::resolve_reference`, which uses
  credential files and never the ambient environment.
- A saved JQL equal to the default is stored as `""`, so a future change to the
  default reaches everyone who never customised it.

## Jira and HTTP gotchas

- Jira search uses Cloud `/rest/api/3/search/jql` + `nextPageToken`, at most 2
  pages of 100, with `truncated` reported. A 404 means "not Jira Cloud".
  Jira's `errorMessages` on a 400 are JQL feedback and are shown to the user,
  bounded.
- The client is built with `redirect::Policy::none()`, because a token only
  goes to its stored URL and never to a redirect target.
- Response bodies are read chunk by chunk under a 4 MiB cap. `bytes()` would
  buffer a chunked body of any size before the check (Codex r2).
- Client deadlines must cover the whole server operation (Codex r1): a
  1Password lookup of up to 30 s, plus 15 s per upstream request. Otherwise
  the browser's timeout replaces the server's specific error.

## Dialog lifecycle (nice-modal)

- A nice-modal dialog that only `hide()`s stays mounted. Reopening it for the
  same key then shows the previous query result, whatever `staleTime: 0` says.
  `ChngStartSessionDialog` calls `remove()` on close.
- Every instance shares the component's modal id, so an in-flight Continue
  from a dismissed instance could resolve the next one. The request is aborted
  on dismiss or unmount and its completion is guarded (Codex r3).

## Remote web

CHNG needs the local server API. `goToChng` is optional on `AppNavigation`, the
rail hides the Filters section without `onChngClick`, and remote navigation
maps `chng` to `/`.

## Contributed by

- vk/2766-chng-category
