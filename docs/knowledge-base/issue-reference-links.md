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

Do not print the route template in agent-facing guidance. An example like
`[VAS-646](/projects/<project_id>/issues/<issue_id>)` teaches the very splicing
that produces dead links; point at the returned `issue_url` value and say to copy
it verbatim. Assert this: a test that the instruction contains no `/projects/`
or `/issues/` literal keeps the template from creeping back.

When asserting that guidance names a tool, match the surrounding phrasing
(`call 'get_issue'`), not the bare name — the instruction always ends with a
`TOOLS:` listing, so `contains("'get_issue'")` passes no matter what the guidance
actually says.

Server instructions are assembled per launch mode, and the modes do not expose
the same tools: orchestrator mode registers no remote-issue tools at all, so
guidance naming `get_issue` points those agents at something uncallable. Derive
the named fallback from the registered router and assert it per mode. Scope what
the guidance promises, too: `get_context` only knows this workspace's own issue,
so orchestrator-mode guidance has to ask for the issue key rather than a link for
any other issue, or it is an instruction that mode cannot satisfy.

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
appears to a user. `cursor: not-allowed` is dead for the same
reason. Real feedback for disabled links needs `pointer-events` dropped plus a
click guard, which changes click-through behaviour — a separate decision.

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

Keep tool descriptions and server instructions saying the same thing. Telling
`create_issue`/`update_issue` to use `issue_url` "for every issue reference"
contradicts the instruction that bans the route from outbound-synced fields —
and those two tools are exactly the ones writing `title` and `description`,
both of which Jira sync pushes.

When a reference cannot be resolved, report null, not an empty string. A
relationship whose related issue lies in another project (creatable, since
`resolve_issue_id` searches every visible project) used to return
`related_simple_id: ""`, leaving the agent with neither a key to print nor a
signal to call `get_issue`. Nullability is what makes the documented fallback
reachable.

`McpContext.issue_url` is serialized even when `None` (its neighbours
`project_id`/`issue_id` behave the same), so `get_context` emits an explicit
`"issue_url": null`. Guidance must say "null is never a link destination"
rather than describing the field as absent, or a literal-minded agent writes
`[VAS-1](null)`.

CI gap worth closing separately: `.github/workflows/test.yml` runs
`packages/remote-web && npm run test` but never `packages/web-core && npm run
test`, even though web-core has a `test` script and a large suite. Frontend
regressions added there — including this task's link-policy tests — do not gate
merges today.

Second CI gap, fixed here: the `backend` paths filter in
`.github/workflows/test.yml` was a hand-kept crate enumeration that had drifted
to 15 of 33 workspace members, `crates/mcp` among the omissions. A change
touching only an omitted crate skipped backend-test/clippy entirely and reported
green — this task's own Rust changes did exactly that on the first CI run.
Because those jobs operate on the whole workspace, the filter should be
`crates/**`, not a list that must be remembered.
