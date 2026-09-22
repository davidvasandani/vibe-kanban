# Clarifications: vk/3fb0-debug-why-vk-mes

Three questions were raised in `spec.md`. All are resolved, each against code
rather than preference.

## C-1 — Should a byte-capped running history signal its truncation?

**Resolved: no truncation signal — but return the surviving messages, not an
empty list.** (Second half revised during code review; see the amendment at
the end of this section.)

Evidence: `crates/utils/src/msg_store.rs:13` caps retained history at
`HISTORY_BYTES = 100000 * 1024` (100 MB) per execution, evicting from the front
in `push`. A single agent turn reaching 100 MB of retained log is a remote edge
case, not the common path this fix targets.

Consequences if it does happen: eviction can drop an `add /entries/<n>` while a
later `replace /entries/<n>` survives. `materialize_entries`
(`crates/services/src/services/normalized_log_cache.rs:91`) applies patches to a
fresh `{"entries": []}` document and returns `CacheError::Patch` if one does not
apply.

**Amendment (code review).** The original resolution accepted mapping that to
`None`, letting the route's `unwrap_or_default()` produce an empty message
list. Review pointed out this contradicts the principle written for this very
task: constitution XXXVIII says "a capped answer is still preferable to no
answer", and losing *every* message because the oldest was evicted is no
answer. The live-store path now falls back to `materialize_entries_lossy`,
which skips non-applying patches and logs the skipped count, so the read
returns the surviving conversation. The stored-sidecar path keeps the strict
form: there, a patch that does not apply means the artifact is corrupt and
must be re-derived rather than partially trusted.

Rejected alternative: adding a `truncated` field to `RecentMessagesResponse`.
That widens a generated TS type and adds a branch for a case this change does
not introduce and is not measurably hitting. The distinct truncation notice that
*does* exist is for legacy historical re-normalization's newest-2,000-message
bound, which is a different mechanism on a different path and is out of scope.

## C-2 — Should `final_message` be populated for a running execution?

**Resolved: yes — latest assistant text so far, no special case.**

`last_assistant_message` (`crates/server/src/routes/execution_processes.rs:123`)
already derives it from the same entry list the response projects, so populating
it for a running turn requires no new code — suppressing it would.

The knowledge base is explicit that this is safe as long as callers do not
overread it: `authoritative-snapshot-stream-handoffs.md` — "Final output is a
reconciliation trigger, not exit evidence. A normalized final assistant response
can arrive before executor/process finalization. It must never be promoted
directly to `completed`." The response carries `status` and `exit_code`
alongside, which are the authority for completion. Recorded as FR-9 so the
non-null-while-running case is documented rather than surprising.

## C-3 — Should `has_more` also mean "this turn is still producing messages"?

**Resolved: no. Keep today's single meaning.**

`has_more` is set in `project_messages` purely from
`messages.len() > limit` — it is a pagination signal about the current
response. Overloading it would change what an existing caller of a *finished*
execution infers, and FR-4 requires finished-read semantics to stay identical.
Liveness is already expressed by `status`, which is a closed domain
(`ExecutionProcessStatus`) and the documented authority for it — consistent with
constitution principle XXX (derive activity from authoritative status) and with
the knowledge base's "Derive activity from the closed status domain".

Also note `MessagesSelection::All` sets `has_more = false` unconditionally;
overloading the flag would have made that branch incoherent for a running turn.
