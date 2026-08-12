# Implementation plan: mobile workspace right drawer

Task: `a12b9b02-6250-42e9-b5b0-220ea5fca2af`

1. Establish the SpecKit constitution and feature workspace, then reconcile the
   generated specification with `SPEC.md` and the recalled project knowledge.
2. Clarify the interaction model: retain the existing `git` state identifier,
   reuse the mobile tab strip, and present that destination explicitly as the
   workspace right sidebar rather than creating a separate overlay/state path.
3. Generate the SpecKit technical plan, supporting artifacts, and dependency-
   ordered tasks; analyze them for constitution or coverage gaps.
4. Add focused shared-navbar tests that describe the missing discoverability:
   the mobile right-sidebar destination has the established mirrored sidebar
   glyph, an accessible name, a selected-state signal, and invokes the mobile
   tab callback.
5. Update the shared mobile tab metadata/rendering to satisfy those tests while
   preserving the `git` identifier consumed by `useMobileActiveTab` and
   `WorkspacesLayout`.
6. Add or adjust workspace-layout coverage only if existing tests do not prove
   that selecting `git` mounts the shared `RightSidebar` for a selected
   workspace and hides the other tab surfaces without unmounting them.
7. Run focused tests, frontend type checking, linting, and repository formatting;
   inspect the narrow mobile rendering and correct any accessibility or overflow
   regressions.
8. Execute the independent Codex diff review, address confirmed findings, and
   repeat verification/review until there are no significant findings.
9. Record the reusable mobile-drawer/tab contract in the project knowledge base,
   update its index, and commit the knowledge changes.
10. Push the completed task branch, open a pull request against the detected
    base branch, wait for required checks as needed, and merge it.
