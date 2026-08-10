# Implementation Plan: Firecrawl MCP Worker Authentication

1. Add the Firecrawl bearer reference to each worker's generic `executorSecretRefs` configuration.
2. Allowlist `FIRECRAWL_BROWSER_AUTH_TOKEN` in the Firecrawl stdio MCP definition using Codex's `env_vars` forwarding mechanism.
3. Keep the private Firecrawl URL in the MCP definition and the bearer value exclusively in 1Password.
4. Validate the changed JSON and Nix host declarations without exposing secret values.
5. Run independent Codex review, address confirmed findings, and repeat until no significant findings remain.

## Follow-up Implementation: Inline MCP Screenshots

1. Extend the shared MCP image-result normalizer to recognize hosted HTTP(S)
   `resource_link` image blocks.
2. Reuse the shared normalizer in Codex's direct app-server completion path.
3. Teach the shared image node and desktop CSP to display HTTP(S) image URLs.
4. Add focused unit tests for hosted image links and rejected resource links.
5. Run formatting and targeted executor/frontend checks.
6. Reuse Firecrawl's bounded artifact store for reusable screenshot artifacts
   and return capability URLs as MCP `resource_link` image blocks.
7. Verify Firecrawl build/tests and the end-to-end MCP screenshot contract.
8. Run independent Codex review and address confirmed findings until none remain.
