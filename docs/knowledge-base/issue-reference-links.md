# Explicit issue references

Tags: `vk/f31e-issue-link`

## Give agents a destination

Issue-facing MCP results expose `issue_url`, an application-relative browser
route built from the record's project and issue UUIDs. The route is shared by
the local and remote web applications: `/projects/{project}/issues/{issue}`.
Do not derive a browser origin from `VIBE_BACKEND_URL`: in clustered deployments
that address is an internal coordinator transport endpoint.

Build every destination from identity that is actually on the record in hand.
`issue_url`, `related_issue_url` and sub-issue URLs qualify; `related_issue_url`
stays null when the authoritative project record is unavailable rather than
guessing.

`parent_issue_id` deliberately ships **without** a URL. A parent can live in a
different project: `create_issue` resolves `parent_issue_id` through
`resolve_issue_id`, which searches every visible project, and the FK
(`parent_issue_id UUID REFERENCES issues(id)`) carries no same-project
constraint. Splicing the parent id into the child's project route yields a link
that looks valid and 404s, which is worse than no link — `ProjectProvider`
resolves issues by exact lookup within one project. `fetch_sub_issues` being
project-scoped proves only that same-project children are *discoverable*, not
that cross-project parents are impossible; do not read it as an invariant.
Agents reach a parent through `get_issue`, which returns its real `issue_url`.

## Scope the guidance to where the route resolves

An application-relative route is only meaningful inside the Vibe Kanban UI.
Telling agents to link "every issue reference" is too broad: the same agent
writes pull request bodies, commit messages and Slack posts, where a
root-relative path resolves against the wrong origin and 404s. Instruct linking
inside Vibe Kanban prose and naming the issue key elsewhere. There is no
authoritative public web origin available to the MCP server, so an absolute URL
cannot be offered without inventing one.

Server instructions are assembled per launch mode, and the modes do not expose
the same tools: orchestrator mode registers no remote-issue tools at all, so
guidance naming `get_issue` points those agents at something uncallable. Derive
the named fallback from the registered router (`get_context` carries the active
issue URL) and assert it per mode.

## Preserve the link through read-only rendering

`packages/ui/src/components/ReadOnlyLinkPlugin.tsx` permits the exact UUID issue
route alongside existing external HTTPS links. Arbitrary relative paths and
protocol-relative URLs stay disabled. Both initial links and mutation updates
use the same policy. Read the URL from the Lexical model: a prior policy pass
may have removed the DOM href. Clear stale disabled attributes/styles when a
link becomes allowed. Use `editor.read` when resolving nodes from DOM elements;
`editor.getEditorState().read` does not provide the active editor that DOM-node
lookup requires.

Match the route case-sensitively, end to end. Issue lookup is an exact match
against the lowercase UUIDs Postgres emits and route params are never
normalised, so neither `/PROJECTS/...` nor an upper-case UUID resolves — an `i`
flag only ever admits dead links. Watch for fixtures that make such a test
vacuous: all-numeric UUIDs make `toUpperCase()` a no-op, so case is asserted
only with letter-bearing UUIDs.

Registering the mutation listener is sufficient on its own: `skipInitialization`
defaults to false, so `LinkNode`s that already exist when the plugin mounts arrive
as 'created' mutations. A second `querySelectorAll('a')` sweep at mount is
redundant, not defensive.

`target="_blank"` is for external links only. An issue route belongs to this
app, and opening it in a new tab cold-reloads the SPA; worse, in the Tauri build
`on_new_window` denies the window and hands the URL to the system browser, which
cannot resolve an app-relative path at all — the desktop user is dropped out of
the app onto a broken page. Keep in-app routes in the current window.

A disabled link carries `role="link"`, `aria-disabled`, and `pointer-events:
none`. Do not add a `title` hint to that branch: `pointer-events: none` stops the
anchor being hit-tested, so neither a tooltip nor the `cursor` style can render,
and a jsdom test asserting the attribute would pass while the affordance never
appears to a user.

Regression tests should import real Markdown into Lexical and inspect anchors,
exercise late plugin mounting and URL changes, and retain unsafe-path coverage.
Backend tests should inspect serialized MCP content and verify that nested
records use their own project identity, with no guessed destination for missing
or cross-project records.

Known limitation, accepted: a clickable issue route is a plain anchor, so
following it is a full document navigation that re-bootstraps the SPA rather
than a client-side route change. No web package has a global anchor-click
interceptor, and the plugin lives in `packages/ui` with no router access, so
client-side routing would mean threading a navigate callback through
`WYSIWYGEditor` and its call sites. The link resolves correctly (the server has
an SPA fallback), so this is a polish item, not a broken destination.
