# Implementation Plan: Concatenate Repeating Lines

1. Confirm the Codex normalizer owns the reported
   `codex review --uncommitted` command rows and inventory both supported Codex
   event formats (`item/started`/`item/completed` and legacy exec events).
2. Add a small repeat tracker to `LogState` containing the shared visible index,
   exact command text, repetition count, latest call ID, and latest completion
   outcome.
3. Add helpers that:
   - recognize the narrowly eligible Codex review command;
   - allocate a new index or reuse the immediately previous successful row;
   - preserve the current count during updates for the same call ID;
   - allow only the latest call ID to complete/update the shared row;
   - render bounded repeat ticks.
4. Route both direct app-server command events and legacy Codex protocol command
   events through the shared allocation/completion helpers.
5. Keep non-review commands, changed commands, interrupted runs, approvals,
   failures, and output updates on their existing behavior.
6. Add focused async normalizer tests for:
   - adjacent completed identical review commands collapsing;
   - add-versus-replace/index behavior;
   - changed and interrupted command runs;
   - non-review commands remaining distinct;
   - a failed repeat remaining visibly failed and ending reuse;
   - bounded repeat rendering;
   - both direct and legacy protocol paths.
7. Run formatting and focused executor tests, then broader repository checks in
   proportion to the touched Rust-only scope.
8. Run an independent Codex diff review, address confirmed findings, and repeat
   verification until no significant findings remain.
9. Update the existing repeated-log-entry and Codex-normalization knowledge
   pages with reusable facts from the shipped implementation, tag task
   `a5f8-concat-repeating`, refresh the index, and commit the knowledge-base
   update.
