# Implementation Plan: VK MCP Management UX

1. **Confirm invariants and component boundaries**
   - Use the existing shared draft/read model, codecs, authentication handlers, test-result indexing, and save bar.
   - Extend `McpServerDialog`'s input/result contract so agent assignments are modal-owned together with the server definition.
   - Keep the settings section as the stateful container and the dialog as the focused edit surface.

2. **Model compatibility for modal selection**
   - Pass MCP-capable profiles and current assignments into the dialog.
   - Derive the provisional `McpServerDefinition` from the current form entry and evaluate each profile against its agent codec/known transport restrictions.
   - Render disabled incompatible choices with an actionable reason.
   - Require at least one compatible assignment before submission.

3. **Make add/edit transactional**
   - Seed dialog-local assignment state on every open (NiceModal instances are reused).
   - For a new server, choose a single sensible compatible default when possible.
   - Return `{ name, entry, assignments }` only on successful submit.
   - Ensure close/cancel returns no result and therefore leaves the settings draft untouched.
   - On rename, atomically remove the old draft entry and insert the returned complete entry.

4. **Reorganize the primary MCP management surface**
   - Replace the inline assignment grid with a compact assignment summary/badges.
   - Restructure the section header/count, helper text, inventory cards, and empty state to follow the supplied management UI hierarchy.
   - Use visible Test, Edit, and Delete actions while retaining gateway Reconnect/Disconnect controls and detailed auth/error results.
   - Preserve conflict resolution and JSON-mode affordances without allowing them to dominate the main inventory.

5. **Update copy and localization**
   - Add English strings for server count, agent selection, assignment validation, compatibility messaging, and concise card summaries.
   - Propagate required keys to all locale files using safe fallback-quality translations or shared neutral wording so runtime keys remain complete.

6. **Add focused automated coverage**
   - Extract pure compatibility/default-assignment logic where useful and cover it with Vitest.
   - Cover dialog result/assignment behavior at the most practical existing test layer; if DOM test infrastructure is absent, test pure state helpers and rely on type checks plus visual verification for composition.
   - Confirm existing MCP codec and shared-draft tests remain green.

7. **Verify and polish**
   - Run Prettier on changed frontend files, focused Vitest suites, web-core TypeScript checking, and relevant linting.
   - Run the repository-required formatter.
   - Start the local app when feasible and inspect desktop/narrow layouts against the screenshots.
   - Review the final diff for unintended backend, generated-file, auth, or persistence changes.
