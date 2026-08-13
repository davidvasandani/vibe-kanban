# Prior Knowledge: Low-Disk Server Metrics Warnings

Searched `wiki/`, `docs/knowledge-base/`, and the prior Cluster Server Metrics
feature records for server metrics, disk/filesystem behavior, collapsed
accordions, configuration, issue creation, and duplicate prevention.

## Relevant findings

1. The current Server Metrics transport is intentionally live only while its
   body is expanded. `CollapsibleSectionHeader` unmounts the body, closing the
   metrics socket. A collapsed rollup therefore needs a small header-owned
   subscriber/query; keeping the whole detail container mounted violates the
   existing collection lifecycle.
2. `workspace-affinity-migration.md` records the established pattern for
   collapsed dynamic metadata: mount a bounded header-owned subscriber outside
   the body, reuse the same TanStack Query key as the expanded container, and
   avoid a label-only endpoint or duplicate polling cadence. Header content
   must truncate without displacing the disclosure control and retain a full
   accessible/title description.
3. The original Cluster Server Metrics design treats absent readings as
   distinct from zero, isolates malformed nodes, retains stale samples only
   within the evidence window, and explicitly does not alter worker health or
   scheduler eligibility. Low-disk classification must preserve those rules:
   do not warn from absent data, do not let one malformed sample blank other
   nodes, label stale facts with their observation time, and do not gate
   dispatch in this task.
4. Filesystems are keyed conceptually by mountpoint and current samples already
   contain total, used, available, filesystem name, and mountpoint. The warning
   should derive from this existing source rather than add another host probe.
5. `mcp-connectivity-testing.md` provides the closest issue-creation precedent.
   It uses the existing optimistic issue mutation, first project status, and
   top-of-column ordering, but also warns that React component state is not a
   sufficient duplicate guard because remounts/reloads reset it. Its external
   in-flight map is suitable for suppressing simultaneous UI submissions only;
   this feature's across-session open-issue uniqueness needs durable server/
   database identity.
6. The same page establishes safe generated-Markdown behavior: backend
   diagnostics are treated as opaque and fenced safely. Disk facts are
   structured rather than opaque, so canonical issue Markdown should be
   generated server-side from validated values and stable labels.
7. Existing idempotency knowledge consistently favors stable identities and a
   persistence-level uniqueness invariant over title matching or result-cache
   checks. For low disk, node ID is the incident identity; filesystem details
   remain evidence in the body, while the deduplication promise is one open
   low-disk issue per node as requested.
8. The project knowledge base contains no existing page specifically about
   Server Metrics alert classification or metrics-to-issue follow-through. A
   focused page should be added after implementation if the shipped seams and
   invariants remain reusable.

## Planning consequences

- Reuse the current metrics query/stream and its node/sample types.
- Extract pure threshold classification and rollup helpers shared by body and
  header presentation.
- Keep warning thresholds backend-owned and expose effective configuration with
  metrics/API data so frontend defaults cannot drift.
- Add a durable, transaction-safe resolve-or-create operation keyed by node ID;
  navigation must use the returned issue identity whether newly created or
  reused.
- Preserve the existing collapsed-body collection policy by implementing only
  the smallest header data subscriber needed for the rollup.
- Keep scheduling changes explicitly out of scope.

