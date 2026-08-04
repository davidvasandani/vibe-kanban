# Implementation Plan: Coordinator Placement Option

1. Inspect the workspace creation request, placement scheduler boundary, UI selector, and existing tests to identify the narrowest additive contract.
2. Add an explicit coordinator-placement field to the Rust request model with backward-compatible deserialization and regenerate the TypeScript API types.
3. Refactor clustered creation placement into a testable decision path that:
   - rejects simultaneous coordinator and worker intent;
   - leaves the initial placement local for coordinator intent;
   - preserves scheduler selection and reservation for automatic and manual worker intent.
4. Add backend tests covering coordinator-local selection, contradictory input, automatic scheduling, and explicit worker behavior at the changed boundary.
5. Add **Coordinator** to the create form's **Run on** selector and serialize its selection as explicit coordinator intent with no worker UUID.
6. Add frontend tests for option visibility and payload mapping, using a small pure helper if that keeps the behavior independently testable.
7. Regenerate shared types and run focused Rust/frontend tests, formatting, type checks, generated-type checks, lint, and broader checks in proportion to the touched code.
8. Run an independent Codex diff review, address confirmed findings, and repeat verification until no significant findings remain.
9. Record reusable clustered-placement knowledge in the project knowledge base, tag it with this task identifier, refresh the index, and commit the knowledge-base update separately as required.
