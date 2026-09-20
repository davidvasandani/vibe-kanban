# Explicit issue references

Tags: `vk/f31e-issue-link`

## Give agents a destination

Issue-facing MCP results expose `issue_url`, an application-relative browser
route built from the record's project and issue UUIDs. The route is shared by
the local and remote web applications: `/projects/{project}/issues/{issue}`.
Do not derive a browser origin from `VIBE_BACKEND_URL`: in clustered deployments
that address is an internal coordinator transport endpoint.

Cover every issue reference in a payload, not just the top-level record. A bare
id an agent cannot turn into a link is the failure this contract exists to
prevent, so `issue_url`, `related_issue_url`, `parent_issue_url` and sub-issue
URLs all travel with their records. Derive each from the identity actually on
the record; `related_issue_url` stays null when the authoritative project record
is unavailable rather than guessing. A parent's project is safe to reuse because
sub-issue discovery is itself project-scoped, so parent and child always share
one project — record that invariant next to the code that leans on it.

## Scope the guidance to where the route resolves

An application-relative route is only meaningful inside the Vibe Kanban UI.
Telling agents to link "every issue reference" is too broad: the same agent
writes pull request bodies, commit messages and Slack posts, where a root-relative
path resolves against the wrong origin and 404s. Instruct linking inside Vibe
Kanban prose and naming the issue key elsewhere. There is no authoritative public
web origin available to the MCP server, so an absolute URL cannot be offered
without inventing one.

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

Scope regex case-insensitivity to the parts that are genuinely case-insensitive.
A blanket `i` flag over the whole route also accepts `/PROJECTS/...`, which the
router does not serve, so the plugin would mark a dead link clickable. Hex digits
take the `i`; the path segments do not. Watch for fixtures that make such a test
vacuous: all-numeric UUIDs make `toUpperCase()` a no-op, so the case is asserted
only with letter-bearing UUIDs.

A disabled link carries `role="link"`, `aria-disabled`, and `pointer-events:
none`. Do not add a `title` hint to that branch: `pointer-events: none` stops the
anchor being hit-tested, so neither a tooltip nor the `cursor` style can render,
and a jsdom test asserting the attribute would pass while the affordance never
appears to a user.

Regression tests should import real Markdown into Lexical and inspect anchors,
exercise late plugin mounting and URL changes, and retain unsafe-path coverage.
Backend tests should inspect serialized MCP content and verify that nested
records use their own project identity, with no guessed destination for missing
records.
