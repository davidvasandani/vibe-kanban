# Tasks: Fix Slack MCP Native-Configuration Conflict

**Feature**: `specs/vk/d893-fix-slack-mcp/`
**Task**: `d893-fix-slack-mcp`

Tasks are dependency-ordered. `[P]` tasks in the same layer touch independent
files, are read-only, or are validation-only and may be completed together.

## Layer 0 - Baseline and Orientation

- [x] T001 Read the feature inputs and current shared MCP implementation before
      editing:
      `specs/vk/d893-fix-slack-mcp/spec.md`,
      `specs/vk/d893-fix-slack-mcp/clarifications.md`,
      `specs/vk/d893-fix-slack-mcp/plan.md`,
      `specs/vk/d893-fix-slack-mcp/research.md`,
      `crates/executors/src/shared_mcp_config.rs`,
      `crates/executors/src/mcp_config.rs`, and
      `crates/executors/default_mcp.json`. Confirm the fix remains in backend
      shared MCP read/canonicalization/materialization unless tests prove an
      adjacent change is required.

- [x] T002 Establish the focused baseline from the repo root with
      `cargo test -p executors shared_mcp_config --lib`. Record any
      pre-existing failure before feature edits and avoid masking unrelated
      failures. Depends on T001.

- [x] T003 [P] Inspect the existing pinned Slack guardrail tests and constants
      in `crates/executors/src/mcp_config.rs` without changing them. Confirm
      the expected contract is still `npx`, `-y`, the
      `davidvasandani/slack-mcp-server` `v1.3.0-vk.2` GitHub release tarball,
      `--transport stdio`, and `SLACK_MCP_XOXP_TOKEN`. Depends on T001.

## Layer 1 - Test-First Reproduction

- [x] T004 Add a failing regression test in
      `crates/executors/src/shared_mcp_config.rs` that drives
      `reconcile_snapshots()` with four native-looking `slack` snapshots:
      Codex and Grok as TOML-derived `mcp_servers` entries, and Claude Code and
      Gemini as JSON-family `mcpServers` entries. Use the real pinned Slack
      release URL and fake token values only. Depends on T002.

- [x] T005 In the regression from T004, assert the screenshot-shaped false
      conflict is fixed by requiring zero conflicts, exactly one reconciled
      `slack` server, four assignments, `source_kind` of `Reconciled`, and
      equal native source fingerprints. First confirm the test fails on the
      current implementation or adjust the fixture to match the persisted native
      shape that actually reproduces the Codex-versus-Claude/Gemini/Grok split.
      Depends on T004.

- [x] T006 Add table-style negative tests on the same `reconcile_snapshots()`
      path proving that Slack still conflicts when one profile changes the
      command, one argument, `--transport stdio`, the release artifact URL, the
      env variable name, or the fake token value. Depends on T005.

## Layer 2 - Canonicalization Fix

- [x] T007 Implement a server-aware read-boundary migration next to
      `canonical_definition()` in
      `crates/executors/src/shared_mcp_config.rs` so the exact former bundled
      `slack-mcp-server@latest` template is compared as the current pinned
      catalog definition. Preserve the stored credential while keeping command
      strings, argument order, env key names, and env values significant for
      every other comparison. Depends on T005.

- [x] T008 Confirm the observed difference is artifact drift rather than an
      executor-specific serialization difference, then constrain the
      Slack-specific migration to the exact known historical template. Do not
      ignore any launch-semantic field or equate arbitrary mutable and pinned
      artifacts. Depends on T007.

- [x] T009 Confirm no generated TypeScript files, API payload types, frontend
      conflict UI, bundled Slack catalog entry, Slack fork tag, launcher digest,
      or MCP documentation are changed unless the implementation proves a
      contract change is necessary. Depends on T007.

## Layer 3 - Write/Read Round Trip

- [x] T010 Add a materialization round-trip test in
      `crates/executors/src/shared_mcp_config.rs`: take the reconciled Slack
      definition, build a `SharedMcpWriteRequest` assigning Codex, Claude Code,
      Gemini, and Grok, run the existing write-planning path for each executor,
      and reconcile the resulting native entries again. Depends on T007.

- [x] T011 In the T010 round-trip test, assert the second reconciliation has
      zero conflicts and one `slack` server with all four assignments. Also
      assert Codex/Grok remain TOML-compatible stdio objects, Claude
      Code/Gemini remain JSON-compatible stdio objects, and the command, pinned
      release argument, `--transport stdio`, and `SLACK_MCP_XOXP_TOKEN` env key
      survive unchanged. Depends on T010.

## Layer 4 - Focused Verification

- [x] T012 Run the focused shared MCP tests:
      `cargo test -p executors shared_mcp_config --lib`. Depends on T006 and
      T011.

- [x] T013 [P] Run the pinned Slack catalog shape and contract test:
      `cargo test -p executors slack_preconfigured_server_matches_the_documented_stdio_contract`.
      Depends on T009.

- [x] T014 [P] Run the pinned Slack immutable artifact test:
      `cargo test -p executors slack_preconfigured_server_pins_an_immutable_fork_artifact`.
      Depends on T009.

- [x] T015 Run repository formatting as required by `AGENTS.md`:
      `pnpm run format`. Depends on T012, T013, and T014.

- [x] T016 If dependencies are installed, run the relevant backend check lane:
      `pnpm run backend:check`. If the worktree is missing dependencies or the
      command cannot run locally, record the exact reason in the final handoff.
      Depends on T015.

## Layer 5 - Independent Review, Knowledge, and Commit

- [x] T017 Run an independent diff review against every acceptance criterion in
      `specs/vk/d893-fix-slack-mcp/spec.md`, with special attention to secret
      handling, over-normalization, untouched pinned Slack fork metadata, and
      the absence of unrelated frontend/generated/deployment changes. Address
      confirmed findings and rerun relevant validation. Depends on T016.

- [x] T018 Update the relevant `docs/knowledge-base/` entry with reusable
      Vibe Kanban project knowledge from implementation, even if
      user-facing integration docs remain unchanged. The user explicitly
      requires this final knowledge-base update, so write it directly without
      adding another approval gate. Depends on T017.

- [x] T019 Commit the implementation and required knowledge-base update
      together. Include only intended task files and do not revert unrelated
      worktree changes. Depends on T018.

- [x] T020 Complete the implementation handoff with validation results, any
      skipped command and reason, whether documentation stayed unchanged or was
      updated, the knowledge-base update from T018, and the commit from T019.
      Depends on T019.

## Parallelization Notes

T003 can run alongside the baseline test after T001 because it is read-only and
targets the pinned Slack contract rather than shared MCP reconciliation. T013
and T014 can run in parallel after T009 because they are independent guardrail
tests. T016 is listed after formatting because `pnpm run backend:check` may be
more expensive and should validate the formatted source state. The
canonicalization, negative conflict tests, and round-trip tests all touch
`crates/executors/src/shared_mcp_config.rs`, so they should be sequenced to
avoid drifting fixtures and assertions.
