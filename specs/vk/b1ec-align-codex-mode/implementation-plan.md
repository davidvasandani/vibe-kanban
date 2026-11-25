# Implementation plan: Align Codex model catalog

1. **Rebase** `vk/b1ec-align-codex-mode` onto `origin/main` (no conflicting
   catalog changes upstream).
2. **Test first.** Update the expected vector in
   `discovered_models_match_current_chatgpt_catalog` to the 10-entry catalog
   from `technical-spec.md`, in screenshot order (version descending, then
   Astra > Sol > Terra > Luna). Run it and confirm it fails.
3. **Catalog.** In `Codex::discover_options`, insert `gpt-6.2-astra`,
   `gpt-6.2-sol`, `gpt-6.1-sol` and `gpt-6.1-terra` (all
   `ultra_reasoning_options`) ahead of the GPT-6 entries. Keep the remaining
   six unchanged, and fix the `.clone()` usage on the final entries.
4. **Verify.** Run the focused test, then `cargo test -p executors`,
   `pnpm run generate-types:check` and `pnpm run format`.
5. **Docs.** Update `wiki/executor-model-catalogs.md`: the selector is the
   target, and the backend gates the catalog by client version (the deployed
   CLI must be at least the version that serves the advertised models). Add a
   task contributor entry and refresh `wiki/INDEX.md`.
6. **Review.** Run `codex review --uncommitted` and iterate on findings.
7. **Ship gate.** Open the PR. **Do not merge** until the operator decides on
   the deployed-CLI blocker: the homelab `pkgs.codex` 0.155.1 rejects the four
   new models.
