# Research — 5276-debug-unrecogniz

## Root cause (verified in source)
`crates/local-deployment/src/container.rs` (worker event poll loop) treated any
`Structured` payload that is not a `LogMsg` as agent stdout. The only producers
of `Structured` are in `crates/worker`, and none of them emits a `LogMsg`. All three are
product metadata (`cancellation_phase`, `stream_error`, `worker_error`). Every
cancellation of a worker-run execution therefore injected
`{"cancellation_phase":"requested"}` into agent stdout. When the graceful
interrupt was not enough, `terminating_process_group` and `killing_process_group`
followed. The Claude normalizer's `ClaudeJson::Unknown` branch
(`crates/executors/src/executors/claude.rs:2609`) turned each one into a
`SystemMessage` "Unrecognized JSON message: …".

"More often" matches the move of all executions onto cluster workers, together with more
stop/interrupt paths: queued follow-ups, session restarts, and affinity migration
(constitution XXII). All of these route through `crates/worker/src/cancellation.rs`.

## Decisions
- **Fix at the consumer, not the producer.** The worker journal is an
  evidence log (XVIII), and phases are legitimately recorded there. The defect is
  in how the coordinator projects them. Changing the protocol would need
  versioned rollout for no gain.
- **Do not filter in executor parsers.** The alternative of teaching
  `ClaudeJson` to ignore `cancellation_phase` would need the same change in every
  executor parser (Codex, Gemini, and so on). It would also couple vendor parsing to product
  metadata, which violates IX.
- **Stderr for diagnostics.** The same loop already uses `LogMsg::Stderr` for
  coordinator-authored worker diagnostics. Every executor already renders stderr
  as an error entry, so no new channel or type is needed.
- **Unknown `Structured` kinds become diagnostics, not stdout.** This keeps future
  metadata out of the agent stream while keeping it visible (XI).

## Alternatives rejected
- Drop all non-`LogMsg` `Structured` payloads silently: this loses
  `worker_error`, which is often the only explanation for a failed launch (XI).
- Add typed variants to `ExecutionEventPayload`: this is a protocol change that needs
  worker/coordinator version coordination. It can come later, and the
  classifier would then simply gain arms.

No new dependencies.
