# Prior knowledge — vk/2eb6-keep-env-vars-un

Sources: `docs/knowledge-base/workspace-environment-inheritance.md` (tags
`6d24-org-env-vars-are`, `vk/b0d4-env-vars-value-f`, `vk/a63c-don-t-obfuscate`)
and `docs/knowledge-base/INDEX.md`. The `wiki/` knowledge base has no env-var or
1Password pages that apply here (its matches cover Connect-based re-auth, which
is a different path).

## What the knowledge base already says

- **Resolver contract (b0d4).** `services::environment_secrets` treats only
  *whole values that start with `op://`* as references. Resolution happens in
  memory at environment preparation, through `op read --no-newline -- <ref>`,
  with a scrubbed `OP_*` environment. A literal `OP_SERVICE_ACCOUNT_TOKEN`
  that is itself a reference is rejected. Resolved values are never persisted.
- **Draft readability (a63c / PR #324).** `OrganizationEnvVarsCard` shows the
  add and edit drafts as text when they start with exactly `op://`. The match
  is case-sensitive and **untrimmed**, and the submitted value is never
  transformed. That page also recorded a rule: "saved rows remain redacted …
  do not retrieve stored or resolved secrets to render references."
- **Remote storage.** Values are encrypted with `state.jwt.encrypt_string`.
  Listing responses never carried values, and only
  `projects.rs::get_project_env_vars` decrypts, for injection into agents.
- **Tests** use synthetic references and a fake CLI. Rendered-DOM Vitest
  tests sit next to the card.

## How this task builds on it (and where it deliberately departs)

- The untrimmed exact-prefix rule is why a reference pasted from 1Password
  (`"op://…"`) stays masked and is saved as a literal. This task replaces the
  rule with one shared normalization: strip whitespace and one matching quote
  pair from whole-value references only. The rule is defined once for the
  resolver and remote (in `api-types`) and mirrored in the UI.
- The a63c rule "do not retrieve stored secrets to render references" is
  **narrowed, not dropped**. A reference is not a secret, and the user asked
  for saved references to stay visible. So the admin-only list decrypts
  server-side and returns only values that normalize to a reference. Literal
  values still never leave the server.
- The SQLx offline cache (`crates/remote/.sqlx`) is keyed by query text. Keep
  the SQL unchanged and move `query_as!` to a private row type.
