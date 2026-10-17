# `/speckit.analyze`: Server Metrics Low-Disk Warnings

Cross-checked `spec.md`, `plan.md`, `tasks.md`, `PRIOR_KNOWLEDGE.md`, and
Constitution v0.28.0 against the current code.

## Findings

1. **ERROR — tasks ordering:** Analyze was originally numbered in the final
   validation phase while the pipeline requires it before implementation.
   Resolved by making it dependency-first T000 and renumbering no stable
   implementation tasks.
2. **ERROR — metrics trust boundary:** The first plan allowed the browser's
   structured disk sample to reach the remote issue service, which cannot prove
   it represents current coordinator metrics. Resolved: the local coordinator
   proxy identifies the requested node, performs a bounded current snapshot,
   derives affected filesystems from server-owned facts and effective
   thresholds, and forwards canonical evidence. The remote server still
   validates authorization and shape.
3. **WARNING — configuration ownership:** The initial wording could put
   environment reads in `node-metrics`, a wire/collector library. Resolved:
   defaults/validation live with the type, while process environment loading
   lives at `ClusterMetricsService` construction.
4. **WARNING — remote mutation convergence:** The plan said to wait for txid
   without naming the established frontend helper. Resolved by requiring the
   existing Electric convergence path before navigation/success.
5. **WARNING — terminal identity constraint:** Postgres cannot express a partial
   unique index whose predicate joins customizable project status rows. The
   planned transaction-scoped advisory lock plus machine-readable metadata and
   same-transaction lookup/create provides durable serialization. This is
   consistent with FR-15; no claim of an impossible cross-table index remains.
6. **WARNING — stale checked-in SpecKit commands:** All seven command files name
   `specs/vk/a5f8-concat-repeating/`, which would overwrite another task.
   Resolved by treating those paths as stale template content and using the
   branch/task-scoped directory throughout. This defect is recorded in plan
   risk notes rather than silently followed.
7. **INFO — requirement coverage:** FR-1–FR-12 map to T001/T002/T008–T010;
   FR-13–FR-21 map to T003/T005–T007/T011; FR-22–FR-23 map to T001/T004;
   FR-24–FR-27 map to T004/T007/T013–T014. No uncovered acceptance criterion
   remains.
8. **INFO — constitution:** Explicit issue creation is compatible with refreshed
   Principle XIX because sampling remains side-effect free, action is explicit,
   evidence is preserved, identity is transactionally deduplicated, and no
   alert state is consumed by scheduling. Principles II, IV, V, VI, and XXIII
   have named implementation and test coverage.

## Result

No blocking gap or constitution violation remains. Ready for
`/speckit.implement`.
