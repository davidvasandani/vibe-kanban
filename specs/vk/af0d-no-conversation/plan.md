# Implementation Plan: Recover Missing Codex Conversations

**Spec**: `./spec.md`
**Status**: Ready

## Technical Context

The change is confined to the Rust `executors` crate. Codex runs through its
typed app-server protocol at the repository-pinned `rust-v0.144.1` tag.
`crates/executors/src/executors/codex/jsonrpc.rs` owns response correlation but
currently converts structured JSON-RPC errors to `std::io::Error`.
`crates/executors/src/executors/codex.rs::launch_codex_agent` owns normal chat
thread start/fork and the common registration/turn-start sequence.

## Architecture & Approach

1. Add an `ExecutorError` variant carrying the vendor-neutral JSON-RPC code,
   message, and optional data. Convert pending RPC errors into it in
   `jsonrpc::await_response`, retaining the existing request label in the
   display context.
2. Add a pure, narrow Codex missing-conversation classifier beside the Codex
   launch logic. Require invalid-request code `-32600`, no contradictory data,
   and a full message match for one of the known absent-rollout/conversation
   templates ending in the requested UUID.
3. In normal chat launch, keep the successful `thread/fork` path. If the fork
   returns the classified missing-conversation error, call `thread/start` with
   the same start parameters and continue through the existing resolved-model,
   registration, collaboration-mode, and `turn/start` path.
4. Log recovery at warning level with the missing and replacement thread IDs,
   without logging prompt content or credentials.
5. Add unit tests for structured error preservation/classification and request
   resolution behavior. Prefer a small helper over constructing a complete
   child app-server process fixture.

## Data Model

No database schema or durable entity changes are required. The new external
thread ID already flows through `LogMsg::SessionId` and
`CodingAgentTurn::update_agent_session_id`, replacing the session's latest
continuation target by existing behavior.

## Contracts

See `./contracts/codex-missing-conversation.md`.

## Research Notes

See `./research.md`.

## Constitution Check

- Principle II: focused unit tests cover the recovery and fail-loud contract.
- Principles III/VI: the existing thread-start and common post-start sequence
  are reused; no parallel executor path is added.
- Principle IX: structured protocol errors are preserved and only a verified
  missing-state response recovers.
- Principles XII/XVIII: existing thread registration and worker-affinity paths
  remain authoritative.

No constitution deviations are required.

## Risks & Dependencies

- Upstream wording has changed across Codex versions. The classifier supports
  only the pinned source form and the exact deployed report, both UUID-bound;
  unknown future wording fails loud until deliberately added.
- `ThreadStartParams` is consumed by fork conversion. Ownership must be arranged
  so fallback retains an equivalent value, likely by cloning the typed params
  if supported or constructing fork params from a clone.
- Broad changes to `ExecutorError` could alter displays. Keep the new variant's
  rendered message compatible with the current `<label> request failed: ...`
  text and test it.
