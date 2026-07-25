# Implementation Plan: Claude Opus 5 Model Support

1. Confirm Anthropic's canonical Opus 5 API identifier and identify which
   executor integrations currently advertise the model.
2. Inspect the prior Opus 4.8 change and all current hard-coded model catalogs
   to establish the smallest consistent change surface.
3. Add Opus 5 to supported executor catalogs, including provider-specific
   identifiers, display labels, and reasoning-name resolution.
4. Extend focused unit tests to lock catalog inclusion and any model/reasoning
   resolution behavior.
5. Regenerate executor schemas if schema descriptions change; do not manually
   edit generated files.
6. Run focused tests, generation checks, compilation checks, and repository
   formatting.
7. Run an independent Codex review, address confirmed significant findings,
   and repeat verification as needed.
8. Record the reusable executor model-catalog maintenance procedure in the
   project knowledge base, refresh its index, and commit that knowledge update.
