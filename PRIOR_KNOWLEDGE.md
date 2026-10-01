# Prior knowledge: linting web-core and remote-web

Task: `vk/848f-lint-packages-we`. This file summarises what the project
knowledge bases (`wiki/` and `docs/knowledge-base/`) already say about this
work. In this stage the knowledge bases were only read, not changed.

## Direct hits

- **`wiki/workspace-sidebar-filtering.md`, "Verification notes"** records the
  gap this task closes. `pnpm run lint` doesn't cover `packages/web-core`,
  and the workaround was
  `npx eslint --no-eslintrc -c .eslintrc.cjs ../web-core/src/<file>` from
  `packages/local-web`. The index line repeats it ("web-core is not covered
  by repo lint"). Both must be updated once this lands.
- **Same page:** `scripts/check-i18n.sh`'s duplicate-key check needs GNU
  `diff`. On hosts without it, it reports duplicates in every locale file.
  That is an environment failure, not a lint result, so don't chase it during
  verification.
- **`docs/knowledge-base/responsive-deployment-identity.md`:** `packages/ui`
  has typecheck and lint scripts but no Vitest lane. That is the precedent
  for per-package lint scripts.

## Constraints from pages that cover code this task touches

- **`docs/knowledge-base/shared-mcp-configuration.md`:** the MCP server
  dialog holds provisional state. NiceModal reuses the mounted component, so
  every open must re-seed every field and assignment, and cancel leaves the
  outer draft alone. The `exhaustive-deps` fix in `McpServerDialog` must keep
  re-seeding tied to open, codec and initial. A late `profiles` update must
  not wipe a form that's already open (constitution X).
- **`docs/knowledge-base/pipeline-settings-editor.md`:** keep a newly saved
  id selected until the refreshed status inventory contains it. Query keys
  include `machineClient.queryScopeKey`. Memoizing the `statuses` fallback
  must not change when the selection effect runs once data is available.
  React-query hashes query keys, so memoizing `reposQueryKey` is safe.
- **`docs/knowledge-base/lazy-loading-normalized-conversation-history.md`
  and `wiki/awaited-stream-settlement.md`:** settled turns are fetched once
  per scope through the settled-entries cache. Moving the loaders in
  `useConversationHistory` to module scope must keep the cache in its
  per-hook ref and must not change which callbacks are recreated.

## Not found

No page covers ESLint configuration, the FSD layer-boundary rules, or the
ban on `eslint-disable` comments. The rule set lives only in
`packages/local-web/.eslintrc.cjs`. That is a candidate topic for the
knowledge-base stage.
