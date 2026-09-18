# Technical specification: 1Password environment variable references

Task: `vk/b0d4-env-vars-value-f`.

## Outcome and scope
Vibe Kanban organization Env Vars accept either literal strings or
complete `op://vault/item/[section/]field` references in the existing value field.
For example, `op://Homelab/7mn7ndpzix7pbdb2llruaygwha/credential` resolves to the
credential when preparing an execution. Existing literal values remain unchanged.
Only Vibe Kanban application code and its directly relevant deployment configuration
are in scope.

## Contract
- Persist the supplied reference through the existing encrypted string storage;
  never replace it with a resolved value in settings or API responses.
- Merge organization values with existing precedence before resolving.
- Authenticate using the effective literal `OP_SERVICE_ACCOUNT_TOKEN` environment
  variable, with the service process environment as fallback when not configured.
  Do not recursively resolve references or use a reference as the bootstrap token.
- Resolve only entire values beginning with `op://`; do not interpolate literal
  strings or execute shell input. Resolve afresh for each execution preparation.
- Apply resolved values to agents and supported script/process launches, including
  worker execution paths through existing environment transport.
- Reference lookup failure, missing credentials, or missing runtime prerequisites
  must stop that launch with an actionable, sanitized error. Never silently supply
  a reference or partial environment to the child.
- Do not expose tokens, reference paths, resolved values, or provider diagnostics
  in errors/logs. Keep existing masking and access controls.
- Explain literal/reference support beside the existing settings value field.

## Implementation direction
Reuse the existing organization environment preparation boundary in
`crates/local-deployment/src/container.rs`; add a small tested resolver at an
appropriate shared layer. Evaluate existing 1Password support before selecting
the provider integration. Keep wire types and database storage unchanged.

## Acceptance and verification
Test literal passthrough, mixed values, token selection,
missing/invalid bootstrap credentials, multiple references, exact returned bytes,
provider failure, bounded/cancelled lookup, and secret-safe errors. Verify all
environment consumers and the UI helper text. Run repository formatting, focused
tests, type/lint checks as feasible, and independent Codex diff review. Document
any unavailable checks explicitly. Record reusable findings, then open and merge
the task PR after review and required checks.
