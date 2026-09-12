# SpecKit Analysis: Workspace Server Affinity and Migration

Cross-check of `spec.md`, `plan.md`, and `tasks.md` against `.specify/memory/constitution.md`.

## Findings

1. **[warning] `data-model.md`, `research.md`, `tasks.md` — durable idempotency mechanism is not yet final.** The constitution requires at-most-once continuation across retries/lost responses. The plan correctly makes T001 select a durable mechanism before implementation, but T003 is worded as though a new migration table is certain. T003 must be interpreted as conditional: use an existing durable execution claim if it provides equivalent operation-identity and result replay; otherwise add the operation table. This decision must be written into `research.md` before T003 proceeds.
2. **[warning] `tasks.md` — non-cluster/local acceptance coverage is implicit.** FR-24 requires local placement to be informational with no migration selector. T018/T023 should include a focused component/manual case proving the selector is absent/disabled and the local label is correct.
3. **[info] `spec.md` / `plan.md` — “affinity” and “current server” remain distinct in the model.** The summary carries assigned and requested identities, and the UI formatter owns their wording. Implementation must not collapse an unavailable explicit request into an incorrect assigned hostname.
4. **[info] Constitution principles XII, XVIII, XXI, and XXII are directly covered.** The route has one coordinator owner; existing scheduler/follow-up rules are reused; stop evidence precedes placement; partial outcomes identify the durable boundary; retries are included in contract/tests.
5. **[info] Requirement coverage is complete.** FR-1–FR-3 map to T011/T015/T016; FR-4–FR-10 and FR-19–FR-20 map to T013–T020; FR-11–FR-18 and FR-22–FR-25 map to T003–T010; generated contracts and full validation map to T012/T021–T025; knowledge capture maps to T026.

## Result

No error-level gaps or constitution violations. Implementation may proceed with the two warnings treated as acceptance checks.
