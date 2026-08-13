# Clarifications: Server Metrics Low-Disk Warnings

## Resolved

1. **Which project receives the issue?** The explicit remote project linked to
   the current workspace/issue context. Server Metrics remains useful without
   one, but its issue action is disabled with an explanation. The UI never
   guesses from an arbitrary project.
2. **What is the durable duplicate key?** Project plus node ID plus the
   low-disk incident kind. This satisfies one open issue per node within the
   project where the operator is working without leaking navigation across
   project boundaries. Filesystem is evidence, not part of incident identity.
3. **Which filesystem facts enter the issue?** Every currently affected
   filesystem on the node. The issue is node-level and should remain actionable
   when root and `/tmp`, or root and a shared mount, cross a boundary together.
4. **Does warning state affect scheduling?** No. The request notes scheduling
   gating as a possible separate issue, and Constitution XIX prohibits metrics
   from becoming lifecycle authority.
5. **Does the collapsed header keep the live stream mounted?** No. It uses the
   smallest cache-sharing/header-owned read consistent with existing accordion
   lifecycle knowledge. The detailed live socket remains expansion-owned.

## Open questions

None.
