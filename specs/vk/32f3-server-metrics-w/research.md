# Research: Server Metrics Low-Disk Warnings

## Existing seams

- `FilesystemSample` already provides mountpoint, device, filesystem type,
  total, used, and available bytes. No new host collection is needed.
- `ClusterMetricsSnapshot` is the authoritative REST fallback and WebSocket
  resnapshot model. Configuration placed there stays synchronized with the
  sample it classifies.
- `RightSidebar` already supports a `headerExtra` rendered outside a collapsed
  section body. Prior knowledge proves this is the correct lifecycle seam.
- Remote issues already carry additive JSON `extension_metadata`, project
  statuses, sort order, creator attribution, and txid mutation responses.

## Decisions

1. **OR thresholds, strict boundaries.** A small filesystem can be dangerous at
   8% while a multi-terabyte filesystem can be safe there; a large filesystem
   can still be dangerous below an absolute reserve. Either rule triggering is
   therefore the conservative interpretation. “Below” remains strict `<`.
2. **Node-level incident identity.** Filesystems can appear/disappear or share
   the same backing device. The operational remediation is host capacity
   hygiene, so `(project,node,kind)` is stable and all affected mounts are
   evidence in one issue.
3. **Advisory transaction lock rather than title search.** Titles are editable
   and races can both observe no match. A lock plus machine metadata makes
   lookup and creation one serialized durable operation.
4. **REST header read, no hidden socket.** The original metrics feature requires
   continuous remote collection to stop when the expanded surface closes. A
   bounded REST observation gives the header evidence without pinning that
   lifecycle.
5. **No scheduler gate.** Metrics availability and severity are observation,
   not health evidence; the request explicitly leaves scheduling policy for a
   separate task.

## Rejected alternatives

- Frontend-only duplicate suppression: lost on reload and cannot cover two
  browsers or transport retry.
- Searching for a title prefix: mutable, localization-sensitive, and racy.
- One issue per mountpoint: creates noisy duplicates for root bind mounts and
  splits one host-level remediation decision.
- Hardcoded frontend thresholds: drifts from deployment policy and makes issue
  evidence disagree with highlighting.
- Keeping `ServerMetricsSectionContainer` mounted while collapsed: retains the
  live WebSocket/collector contrary to the established lifecycle contract.

## Dependencies

No new crate or npm dependency is required.
