# Implementation Plan: Reliable Workspace Issue Breadcrumbs

Task: `vk/719f-vk-workspace-iss`

1. Inspect the current navbar breadcrumb data flow and the reachable prior fix
   identified by the project knowledge base.
2. Refresh the SpecKit constitution and create the feature's SpecKit artifacts.
3. Add or retain a pure workspace breadcrumb builder with explicit issue
   resolution states: none, loading, resolved, and unavailable.
4. Update `NavbarContainer` to consume the project-issue query's loading signal
   and map the linked workspace relationship into the correct builder state.
5. Add focused unit coverage for:
   - resolved labels, order, and issue navigation;
   - deferral during linked-issue loading;
   - settled unavailable behavior without navigation or UUID leakage;
   - unchanged unlinked workspace behavior.
6. Run the focused tests, frontend typecheck, formatter, and relevant
   repository checks; repair any failures caused by the change.
7. Run an independent Codex diff review and address findings until no
   significant issues remain.
8. Update the existing workspace-navbar-breadcrumb knowledge page with this
   task id, refresh its index entry if needed, and commit the knowledge-base
   update before handoff.
