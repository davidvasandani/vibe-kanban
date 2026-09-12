# SpecKit analysis: Lazy-load workspace chat history

## Findings

1. **ERROR — `spec.md` / `research.md` / `plan.md`: legacy-history cost is
   inconsistent.** FR-1/FR-2 and the repo-root non-functional requirements say
   opening history and backend work are bounded. `research.md` permits a legacy
   process to perform a full normalization on first access. The requirements
   need an explicit rollout/materialization readiness rule: either prebuild
   legacy transcripts before serving the feature, or surface a distinct
   one-time preparation state without claiming bounded open. This must be
   resolved before product implementation.
2. **WARNING — `contracts/history-api.md`: live revision ownership is described
   but not fully specified.** The contract needs to state whether revisions are
   assigned before or after durable materialization, what a client receives on
   broadcast lag, and how resume retention is bounded. F003 must close this
   before code lands.
3. **WARNING — `data-model.md`: storage placement and atomicity remain design
   choices.** The logical model is sufficient for requirements, but the product
   plan must choose SQLite rows versus an atomic sidecar/index and specify the
   transaction/crash boundary before F001.
4. **WARNING — `spec.md`: semantic-boundary allowance is only in
   `clarifications.md`.** A server returning fewer than the requested entry
   count is compatible with the API, but the spec should make clear that page
   size is a maximum and clients must rely only on `has_more`/cursor.
5. **INFO — constitution coverage is otherwise complete.** The artifacts keep
   the feature in shared `web-core`, preserve normalized entry identity, require
   explicit snapshot/live ownership, enforce server bounds/cancellation, avoid
   generated-file hand edits, and do not request homelab or other-service
   changes.
6. **INFO — task scope matches the user request.** `tasks.md` distinguishes the
   investigation/requirements deliverable from a follow-up product-code feature
   while retaining an implementation-ready dependency order.

## Result

Findings 1 and 4 were resolved during the implementation stage: legacy
transcripts now require observable, capacity-bounded rollout materialization
outside interactive page requests, and page size is explicitly a maximum whose
exhaustion is determined only by cursor/`has_more` state.

The requirements package is ready for follow-up product implementation.
Findings 2 and 3 remain explicit design gates for F003 and F001 respectively;
they are not ambiguities in the requested requirements deliverable and must be
closed before those product tasks land.
