# MCP incident research — vk/669e-mcp-blocks-progr

Execution 32ece005-0faa-4919-9225-b5b33e08ca49, session dac05324-8cf7-48e4-aba0-3b16fec2489d, worker think5. Started 2026-09-18 14:10:16.536799857Z, classified indeterminate 14:10:37.790932551Z with no exit code.

Coordinator journal at 14:10:37.785878Z reports requested_after=41 and earliest_available=5358. Worker EventJournal retains 4096 events; stream_output splits bytes into at most 8192-byte events. Coordinator replay-gap handling cannot recover a terminal outcome from a still-running worker and closes tracking as indeterminate.

Retained raw output contains an explicit full-history hydration deprecation warning recommending excludeTurns=true. Its final stdout chunk is an incomplete JSON-RPC response id=3, result.thread holding the newly forked thread. This follows account lookup and the fork request; no assistant response has arrived. MCP transport and 401 errors precede that oversized response but are not the coordinator's termination trigger.

Pinned protocol source: openai/codex rust-v0.144.1, commit 44918ea, app-server-protocol/src/protocol/v2/thread.rs. ThreadForkParams and ThreadResumeParams provide the experimental boolean exclude_turns serialized as excludeTurns. app-server/src/request_processors/thread_processor.rs sets include_turns=!exclude_turns but still reads persisted history for the model's fork. VK's handle_jsonrpc_response consumes thread ID/model/reasoning only, not historical turns.

The incident ran Codex 0.154.0. No version bump or speculative MCP endpoint repair is required. Preserve the existing unknown-outcome evidence and original stderr. No new dependencies or data-model/contracts are needed.

## Validation and review

- Locked dependency installation and full `pnpm run format` completed successfully.
- Isolated Codex 0.154.0 app-server probe: initialized with experimentalApi, resumed synthetic history containing a 1 MB assistant message, then forked; both excludeTurns requests succeeded with empty thread.turns and metadata responses below 1.5 KB. No model turn, real workspace continuation, or external service mutation was performed. This verifies response behavior, not model recall; history-input preservation is covered by Rust regression and upstream source semantics.
- Independent `codex review --uncommitted`: no actionable regressions. Reviewer did not execute tests; test execution is tracked separately.
- Initial Rust test build exhausted the host root filesystem. Task-owned build output was moved to shared storage and the build restarted there with bounded compilation concurrency and temporary files on shared storage.
