# Implementation Plan: Firecrawl MCP Worker Authentication

1. Add the Firecrawl bearer reference to each worker's generic `executorSecretRefs` configuration.
2. Allowlist `FIRECRAWL_BROWSER_AUTH_TOKEN` in the Firecrawl stdio MCP definition using Codex's `env_vars` forwarding mechanism.
3. Keep the private Firecrawl URL in the MCP definition and the bearer value exclusively in 1Password.
4. Validate the changed JSON and Nix host declarations without exposing secret values.
5. Run independent Codex review, address confirmed findings, and repeat until no significant findings remain.
