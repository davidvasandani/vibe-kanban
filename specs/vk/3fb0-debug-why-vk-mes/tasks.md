# Tasks: vk/3fb0-debug-why-vk-mes

**Plan**: `./plan.md`

Nearly all work lands in one file
(`crates/services/src/services/container.rs`), so `[P]` is rare by nature —
parallel-safe tasks are marked, everything else is sequential on that file.

## Layer 1 — Refactor (no behaviour change)

- [x] **T001** Lift the buffered-patch filter out of
  `cache_execution_from_history` into a free function
  `indexed_entry_patches_from_history(msg_store: &MsgStore) -> Vec<Patch>`, and
  call it from `cache_execution_from_history`.
  *File*: `crates/services/src/services/container.rs`
  *Verify*: `cargo test -p services` still green; no diff in behaviour.

- [x] **T002** Lift the shared tail of `normalized_entries` (materialize →
  deserialize → warn on failure) into
  `entries_from_patches(id: &Uuid, patches: &[Patch]) -> Option<Vec<NormalizedEntry>>`
  and have the existing stream-drain branch call it.
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T001 (same region of the file)

## Layer 2 — The fix

- [x] **T003** Add
  `normalized_entries_from_history(id: &Uuid, msg_store: &MsgStore) -> Option<Vec<NormalizedEntry>>`
  composing T001 + T002. Document *why* it exists: a live store's stream is a
  tail that only ends when the turn does.
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T001, T002

- [x] **T004** In `ContainerService::normalized_entries`, return
  `normalized_entries_from_history` when `get_msg_store_by_id` yields a live
  store; otherwise keep the existing stream drain. Comment the ordering
  agreement with `stream_normalized_logs`.
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T003

## Layer 3 — Regression coverage

- [x] **T005** Test: a store with buffered entry patches, **no** `Finished`
  pushed and still alive, yields those entries via
  `normalized_entries_from_history`, inside `tokio::time::timeout` so a
  reintroduced wait fails as a timeout rather than hanging the suite. (FR-1,
  FR-2)
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T003

- [x] **T006** `[P]` Test: the pre-fix stream shape
  (`history_plus_stream().filter(..).chain(once(Finished))`) does **not**
  terminate while the store is alive — proving T005's deadline is load-bearing
  and documenting the hazard.
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T003

- [x] **T007** `[P]` Test: repo-diff patches (`/entries/<repo>/<file>`) are
  excluded from a running store's entries, and a store with no patches returns
  an empty list rather than blocking or erroring. (FR-8, contracts invariant)
  *File*: `crates/services/src/services/container.rs`
  *Depends on*: T003

## Layer 4 — Verification

- [x] **T008** `cargo test --workspace`. Confirm pre-existing
  finished-execution tests
  (`a_cached_normalized_entry_survives_the_read_pipeline_normalized_entries_uses`,
  `a_finished_turn_writes_its_sidecar_without_anyone_opening_logs_first`,
  `an_empty_store_writes_no_sidecar`, `project_messages` tests) still pass
  unchanged. (FR-4)
  *Depends on*: T005–T007

- [x] **T009** `[P]` `pnpm run check` and `pnpm run lint`.
  *Depends on*: T004

- [x] **T010** `[P]` `pnpm run generate-types:check` — must be a no-op, proving
  the wire contract is unchanged.
  *Depends on*: T004

- [x] **T011** `pnpm run format`.
  *Depends on*: T008, T009, T010

## Review outcome (T012)

Codex CLI is installed (0.155.1) but unauthenticated in this environment, so
the independent review ran through the `code-review` skill instead. It
confirmed the diagnosis independently and raised three findings, all real and
all fixed:

1. **The regression test was vacuous.** It wrapped a *synchronous* call in
   `std::future::ready` inside `tokio::time::timeout`, so the deadline could
   never fire, and it never exercised the source choice — deleting the fix left
   it green. Fixed by extracting `normalized_entries_from_sources` and asserting
   against a never-yielding fallback stream. Verified by sabotage: removing the
   live-store branch now fails the test with `Elapsed`.
2. **Eviction collapsed the read to empty.** Strict materialization abandons
   the whole document when an evicted `add` orphans a `replace`. Added
   `materialize_entries_lossy` for the live-store path only.
3. **Per-poll whole-history clone under the lock.** `get_history()` deep-copied
   all retained history — mostly stdout — blocking `push`. Added
   `MsgStore::select_history` to clone only the selected variant.

### Second review round

Re-running the review on the fixes found that fix 2 above did not actually
work, and that its test hid the failure:

4. **The lenient pass recovered nothing in the case it was written for.**
   Skipping non-applying patches *in place* is useless after front eviction:
   once `add /entries/0..k` are gone, every surviving `add /entries/N` is
   itself out of bounds, so all of them are skipped too — the read returned
   zero entries while logging that it was serving survivors. Replaced with
   `materialize_entries_rebased`, which appends each surviving `add` and
   remaps later `replace`/`remove` onto its new position.
5. **The eviction test used an ordering no normalizer emits.** It put
   `replace /entries/0` *before* `add /entries/0`; real patch runs have
   monotonic indices with every replace after its own add. Rewritten as a
   genuine front-eviction run starting at index 2, plus direct unit tests for
   the re-basing pass covering replace, remove-with-shift, orphaned
   operations, and the invariant that an intact history re-bases to exactly
   what strict application produces.

## Layer 5 — Close-out

- [x] **T012** Independent Codex review of the diff; address confirmed findings
  and re-verify. (pipeline stage 11)
  *Depends on*: T011

- [x] **T013** Fold the running-execution rule into the knowledge base page that
  already owns this area — the "MCP settled-projection reads" section of
  `docs/knowledge-base/lazy-loading-normalized-conversation-history.md` — tag it
  with this task id, and refresh `docs/knowledge-base/INDEX.md`.
  *Files*: `docs/knowledge-base/lazy-loading-normalized-conversation-history.md`,
  `docs/knowledge-base/INDEX.md`
  *Depends on*: T012
