# Technical Spec: Firecrawl Browser MCP Smoke Test

## Objective

Verify that the Firecrawl browser MCP made available to this Vibe Kanban task can load `https://admin13.parpos.com/` and return enough page evidence to confirm the browser request completed.

When no user follow-up already exists, use:

- Invoke the configured Firecrawl browser MCP from the active Vibe Kanban task.
- Navigate to the supplied HTTPS URL.
- Record whether the MCP is available, whether navigation succeeds, the final URL, and a concise description of returned page content or any error.
- Keep the test read-only: do not submit forms, authenticate, or interact with page controls.

## Out of Scope

- Source-code or deployment changes to Vibe Kanban.
- Changes to `modules/vibe-kanban-rebuild.nix` or any other service.
- Troubleshooting or modifying the target website.
- Bypassing authentication, certificate, or access controls.

## Acceptance Criteria

1. The Firecrawl browser MCP is invoked specifically, rather than a generic HTTP client or alternate browser.
2. It attempts to load exactly `https://admin13.parpos.com/`.
3. The result reports success with page evidence, or reports the precise MCP/navigation failure.
4. No state-changing interaction occurs on the target website.
5. An independent Codex review finds no significant issue in the task diff.
