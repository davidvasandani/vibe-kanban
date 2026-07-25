# Prior Knowledge: Workspace Issue Breadcrumbs

Knowledge-base search terms: `breadcrumb`, `Issue unavailable`, `issue
identity`, `useShape`, `Electric`.

## Direct match

`wiki/workspace-navbar-breadcrumbs.md` describes the exact asynchronous
identity boundary involved in this task:

- `NavbarContainer.tsx` owns entity lookup, loading interpretation, and
  navigation; `packages/ui` should receive prepared breadcrumb items only.
- A remote workspace's non-null `issue_id` is relationship truth. A missing
  row in a not-yet-ready project-issues collection does not mean the workspace
  is unlinked or the issue is unavailable.
- The linked UUID is for lookup and routing. The visible label must be the
  issue's `simple_id`.
- Breadcrumb resolution should use explicit `none`, `loading`, `resolved`, and
  `unavailable` states.
- While loading a linked issue, defer the trail. Once settled, render either
  the resolved linked issue or the non-interactive `Issue unavailable`
  placeholder.
- A pure breadcrumb builder is the preferred test seam. Tests should include
  negative invariants for raw UUID leakage, partial linked hierarchy, and
  unavailable-item navigation.
- `useShape.isLoading` is the relevant initial-query signal; collection
  emptiness alone is ambiguous.

## Supporting match

`wiki/electric-sync-fallback.md` confirms that Electric-backed collections can
be temporarily unready and can transition to REST fallback. This task should
classify the consumer's loading state correctly rather than modify shared
Electric fallback/recovery behavior.

## Consequence for this task

Implement the correction in web-core's navbar container and its pure helper.
Do not redesign the navbar, expose UUIDs, or change Electric synchronization.
The existing knowledge strongly suggests this regression has already been
solved on another line of development, so the implementation stage should
compare reachable history and reuse the proven patch where it applies.
