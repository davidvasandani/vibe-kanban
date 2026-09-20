# Explicit issue references

Tags: `vk/f03e-issue-link`

## Give agents a destination

Issue-facing MCP results expose `issue_url`, an application-relative browser
route built from the record's project and issue UUIDs. The route is shared by
the local and remote web applications: `/projects/{project}/issues/{issue}`.
Do not derive a browser origin from `VIBE_BACKEND_URL`: in clustered deployments
that address is an internal coordinator transport endpoint.

MCP server instructions and issue tool descriptions require Markdown links for
issue references. Use the returned destination; retrieve missing metadata with
`get_issue`. Related-issue links remain null when their authoritative project
record is unavailable. External MCP consumers need their configured web origin
to resolve application-relative links. This contract does not rewrite historical
bare-key prose or force an agent to follow its instructions.

## Preserve the link through read-only rendering

`packages/ui/src/components/ReadOnlyLinkPlugin.tsx` permits the exact UUID issue
route alongside existing external HTTPS links. Arbitrary relative paths and
protocol-relative URLs stay disabled. Both initial links and mutation updates
use the same policy. Read the URL from the Lexical model: a prior policy pass
may have removed the DOM href. Clear stale disabled attributes/styles when a
link becomes allowed. Use `editor.read` when resolving nodes from DOM elements;
`editor.getEditorState().read` does not provide the active editor that DOM-node
lookup requires.

Regression tests should import real Markdown into Lexical and inspect anchors,
exercise late plugin mounting and URL changes, and retain unsafe-path coverage.
Backend tests should inspect serialized MCP content and verify that nested
records use their own project identity, with no guessed destination for missing
records.
