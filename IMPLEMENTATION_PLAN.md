# Implementation plan: default workspace bases to remote mainline

<<<<<<< HEAD
1. Reuse the existing `resolveDefaultBranch` policy in every repository branch
   selection hook used to assemble new-workspace inputs.
2. Preserve explicit initial-branch precedence, then delegate repository
   default and fallback selection to the canonical helper.
3. Add focused hook tests proving `origin/main`/`origin/master` outrank a current
   deployment branch while configured and explicit initial choices still win.
4. Retain exact remote-prefixed target branch names through workspace input
   creation; make no `/srv/src` checkout or deployment changes.
5. Run formatting, frontend tests, type checking, and linting.
6. Run SpecKit analysis and implementation tasks, then independent Codex review
   until no significant findings remain.
7. Update the existing branch-defaulting knowledge page and index contribution
   metadata, commit the knowledge update, and merge into the base branch.
=======
1. Add the Firecrawl bearer reference to each worker's generic `executorSecretRefs` configuration.
2. Allowlist `FIRECRAWL_BROWSER_AUTH_TOKEN` in the Firecrawl stdio MCP definition using Codex's `env_vars` forwarding mechanism.
3. Keep the private Firecrawl URL in the MCP definition and the bearer value exclusively in 1Password.
4. Validate the changed JSON and Nix host declarations without exposing secret values.
5. Run independent Codex review, address confirmed findings, and repeat until no significant findings remain.

## Follow-up Implementation: Inline MCP Screenshots

1. Extend the shared MCP image-result normalizer to fetch hosted HTTP(S)
   `resource_link` image blocks with bounded time and size, rejected redirects,
   and destination validation with an explicit private-origin allowlist.
2. Persist successful downloads content-addressed in `.vibe-attachments/` and
   emit only worktree-relative Markdown references.
3. Reuse the shared normalizer in Codex's direct app-server completion path.
4. Add focused tests for successful import, transfer failure, invalid response
   type, oversized content, and rejected resource links.
5. Run formatting and targeted executor/frontend checks.
6. Reuse Firecrawl's bounded artifact store for reusable screenshot artifacts
   and return capability URLs as MCP `resource_link` image blocks.
7. Verify Firecrawl build/tests and the end-to-end MCP screenshot contract.
8. Run independent Codex review and address confirmed findings until none remain.
>>>>>>> 0e5e0ea7 (fix: persist hosted MCP images locally)
