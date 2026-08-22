# Technical Plan: `list_all_messages`

**Spec**: `./spec.md`
**Status**: Ready for tasks

## Technical Context

The feature is Rust-only. `crates/server` exposes Axum JSON routes over the
normalized-log projection provided by `crates/services`; `crates/mcp` uses
`rmcp` tool macros and `reqwest` to call those routes. No database schema,
generated TypeScript contract, frontend, dependency, or deployment change is
required.

## Architecture & Approach

In `crates/server/src/routes/execution_processes.rs`, extend
`RecentMessagesQuery` with an optional/defaulted `all` flag. Replace the
builder's raw `usize` limit parameter with an explicit selection enum (bounded
tail or all) so call sites cannot encode “all” as a magic number. Filtering and
materialization stay shared; only the final tail selection differs.

Both execution and session routes derive that selection from the query. Recent
requests continue to pass through `clamp_message_limit`; `all=true` bypasses
only response tailing, not normalized-history reconstruction safeguards.

In `crates/mcp/src/task_server/tools/sessions.rs`, add a request type without a
limit, factor target resolution plus owning-workspace authorization into a
shared helper, and generalize the HTTP query helper around an enum that emits
either `limit=N` or `all=true`. Both MCP tools convert the same payload into the
same response shape.

Add `list_all_messages` to the sessions tool router through the existing
`#[tool_router]` implementation and update the explicit orchestrator tool-name
test in `crates/mcp/src/task_server/tools/mod.rs`. Update
`crates/mcp/AGENTS.md` with caller guidance and the legacy projection boundary.

## Data Model

See `./data-model.md`. No persistent entities change.

## Contracts

See `./contracts/messages-api.md` and `./contracts/mcp-tool.md`.

## Research Notes

See `./research.md`. No new dependency is introduced.

## Constitution Check

- **I / III / VI**: use a named selection enum and shared existing projection;
  no new store, parser, or duplicated route.
- **II**: focused tests cover the >100-message distinction, filtering, ordering,
  and router exposure.
- **IX**: settled normalized patches remain the source; no raw slicing.
- **XIX / XXXI**: the read remains side-effect-free, single-flight on cache
  misses, atomically materialized, and bounded during legacy reconstruction.
- Existing recent-reader behavior is preserved, generated files are untouched,
  no dependency is added, and repository formatting runs before completion.

No constitution deviation or open question remains.

## Risks & Dependencies

- **Misleading completeness**: docs and schema call this the complete available
  normalized projection and disclose the legacy reconstruction cap.
- **Accidental recent-reader regression**: keep clamping in the route selector
  and test both modes against the same >100-entry fixture.
- **Authorization drift**: one MCP target/authorization helper serves both
  tools.
- **Oversized MCP payloads**: entry count may be large by design, but individual
  text remains truncated and legacy reconstruction remains bounded.

## Verification

1. Run focused server message-response tests.
2. Run focused MCP tool-router and sessions tests.
3. Run `cargo fmt --all -- --check`, relevant crate checks/tests, and the
   repository-mandated formatter after locked dependency setup if needed.
4. Inspect the diff and run independent Codex review until clean.
